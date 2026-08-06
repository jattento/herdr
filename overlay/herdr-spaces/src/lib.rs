//! Codex-style spaces for herdr: a space is a user-defined group of folders
//! with a name and an optional emoji.
//!
//! This crate owns the model, persistence, sidebar grouping rule, and the
//! picker state machine. It never depends on herdr; herdr calls in and
//! translates the returned plain enums into its own actions.

pub mod flow;
pub mod grouping;
pub mod model;
pub mod store;
pub mod view;

use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub use flow::{Action, Env, FsEnv, Key, PickerState, Row, RowKind, Step};
pub use grouping::{header_label, header_run, space_index_for_path, space_index_for_workspace};
pub use model::{Space, SpacesFile};
pub use view::{Segment, Tone};

/// A cheap (mtime, len) stat of `spaces.json`, used to notice out-of-process
/// edits without a filesystem watcher thread. `Absent` is cached explicitly
/// (not just `None`) so a delete is distinguished from "never checked", and
/// a later re-create is detected just like an edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Fingerprint {
    #[default]
    Absent,
    Present { modified: SystemTime, len: u64 },
}

impl Fingerprint {
    /// One `stat` call. Never fails: an unreadable or missing path is
    /// `Absent`, matching `store::load`'s own missing-file handling.
    fn stat(path: &Path) -> Self {
        match std::fs::metadata(path) {
            Ok(meta) => Self::Present {
                modified: meta.modified().unwrap_or(std::time::UNIX_EPOCH),
                len: meta.len(),
            },
            Err(_) => Self::Absent,
        }
    }
}

/// Loaded spaces plus the open picker, if any. Held as one field on herdr's
/// `AppState` so the fork touches the state struct exactly once.
#[derive(Debug, Default)]
pub struct SpacesState {
    pub list: Vec<Space>,
    pub picker: Option<PickerState>,
    file: PathBuf,
    fingerprint: Fingerprint,
}

impl SpacesState {
    /// Load `<config_dir>/spaces.json`. A missing file yields an empty list.
    pub fn load(config_dir: &Path) -> Self {
        let file = config_dir.join(store::FILE_NAME);
        let fingerprint = Fingerprint::stat(&file);
        Self {
            list: store::load(&file),
            picker: None,
            file,
            fingerprint,
        }
    }

    /// Re-stat `spaces.json` and reload it if it changed (edited, deleted,
    /// or created) since the last load or reload. Herdr's app lives in a
    /// long-running server process, so this is what makes a hand-edited
    /// file visible without a restart; call it on a coarse poll rather than
    /// per-frame. One `stat` per call — the file is only re-read (via the
    /// same `store::load`, corrupt-rename included) when the stat differs.
    /// Returns whether a reload happened.
    pub fn maybe_reload(&mut self) -> bool {
        let fingerprint = Fingerprint::stat(&self.file);
        if fingerprint == self.fingerprint {
            return false;
        }
        self.fingerprint = fingerprint;
        self.list = store::load(&self.file);
        // A picker built from the old list could show stale rows or an
        // index that no longer resolves, so the simplest safe behavior is
        // to close it; callers that need one open a fresh one on demand.
        self.picker = None;
        true
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn env(&self) -> FsEnv {
        FsEnv::new(self.file.clone())
    }

    /// Feed a key press to the open picker. Returns [`Action::None`] when no
    /// picker is open.
    pub fn on_key(&mut self, key: Key) -> Action {
        let env = self.env();
        let Some(picker) = self.picker.as_mut() else {
            return Action::None;
        };
        let action = picker.on_key(key, &mut self.list, &env);
        if env.saved() {
            // The picker wrote spaces.json itself; adopt the new fingerprint
            // so the periodic maybe_reload poll does not treat our own write
            // as an external edit and reset the open picker.
            self.fingerprint = Fingerprint::stat(&self.file);
        }
        action
    }

    /// Rows of the open picker, for rendering.
    pub fn rows(&self) -> Vec<Row> {
        self.picker
            .as_ref()
            .map(|picker| picker.rows(&self.list))
            .unwrap_or_default()
    }

    /// Composed modal body for the open picker, padded to `width`.
    pub fn picker_lines(&self, width: u16, max_rows: usize) -> Vec<Vec<Segment>> {
        self.picker
            .as_ref()
            .map(|picker| view::body_lines(picker, &self.list, width, max_rows))
            .unwrap_or_default()
    }

    /// Space owning this workspace, by worktree repo root then identity cwd.
    pub fn space_for_workspace(
        &self,
        repo_root: Option<&Path>,
        identity_cwd: &Path,
    ) -> Option<(usize, &Space)> {
        let idx = space_index_for_workspace(&self.list, repo_root, identity_cwd)?;
        self.list.get(idx).map(|space| (idx, space))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_yields_an_empty_state() {
        let dir = std::env::temp_dir().join("herdr-spaces-state-missing");
        let state = SpacesState::load(&dir);
        assert!(state.is_empty());
        assert!(state.picker.is_none());
        assert_eq!(state.rows(), Vec::new());
    }

    #[test]
    fn keys_without_a_picker_do_nothing() {
        let mut state = SpacesState::load(Path::new("/nonexistent-herdr-spaces"));
        assert_eq!(state.on_key(Key::Enter), Action::None);
    }

    #[test]
    fn workspace_lookup_uses_the_loaded_list() {
        let mut state = SpacesState::load(Path::new("/nonexistent-herdr-spaces"));
        state.list.push(Space {
            id: "one".into(),
            name: "keyway".into(),
            emoji: None,
            folders: vec![PathBuf::from("/work")],
        });
        let found = state.space_for_workspace(None, Path::new("/work/repo"));
        assert_eq!(
            found.map(|(idx, space)| (idx, space.name.clone())),
            Some((0, "keyway".to_string()))
        );
        assert!(state
            .space_for_workspace(None, Path::new("/elsewhere"))
            .is_none());
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0);
            let dir = std::env::temp_dir().join(format!("herdr-spaces-state-{tag}-{nanos}"));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            Self(dir)
        }

        fn file(&self) -> PathBuf {
            self.0.join(store::FILE_NAME)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn one_space() -> Vec<Space> {
        vec![Space::new("keyway", None, PathBuf::from("/work/keyway"))]
    }

    fn two_spaces() -> Vec<Space> {
        vec![
            Space::new("keyway", None, PathBuf::from("/work/keyway")),
            Space::new("second-space", None, PathBuf::from("/work/second")),
        ]
    }

    #[test]
    fn maybe_reload_detects_an_edit_and_updates_the_list() {
        let dir = TempDir::new("edit");
        store::save(&dir.file(), &one_space()).expect("save");
        let mut state = SpacesState::load(&dir.0);
        assert_eq!(state.list.len(), 1);

        store::save(&dir.file(), &two_spaces()).expect("save");

        assert!(state.maybe_reload());
        assert_eq!(state.list.len(), 2);
    }

    #[test]
    fn maybe_reload_is_a_no_op_when_the_file_is_unchanged() {
        let dir = TempDir::new("unchanged");
        store::save(&dir.file(), &one_space()).expect("save");
        let mut state = SpacesState::load(&dir.0);

        assert!(!state.maybe_reload());
        assert_eq!(state.list.len(), 1);
    }

    #[test]
    fn maybe_reload_detects_deletion_and_reloads_to_empty() {
        let dir = TempDir::new("deleted");
        store::save(&dir.file(), &one_space()).expect("save");
        let mut state = SpacesState::load(&dir.0);
        assert!(!state.is_empty());

        std::fs::remove_file(dir.file()).expect("remove");

        assert!(state.maybe_reload());
        assert!(state.is_empty());
    }

    #[test]
    fn maybe_reload_detects_a_file_created_after_start() {
        let dir = TempDir::new("created-after-start");
        let mut state = SpacesState::load(&dir.0);
        assert!(state.is_empty());

        store::save(&dir.file(), &one_space()).expect("save");

        assert!(state.maybe_reload());
        assert_eq!(state.list.len(), 1);
    }

    #[test]
    fn maybe_reload_closes_an_open_picker() {
        let dir = TempDir::new("picker-reset");
        store::save(&dir.file(), &one_space()).expect("save");
        let mut state = SpacesState::load(&dir.0);
        state.picker = Some(PickerState::new());

        store::save(&dir.file(), &two_spaces()).expect("save");

        assert!(state.maybe_reload());
        assert!(state.picker.is_none());
    }

    #[test]
    fn a_picker_driven_save_does_not_reset_the_picker_on_the_next_poll() {
        // Regression: creating a space from inside the picker writes
        // spaces.json; the next maybe_reload poll must not mistake that for
        // an external edit and close the picker mid-flow.
        let folder = TempDir::new("picker-save-folder");
        let dir = TempDir::new("picker-save-config");
        let mut state = SpacesState::load(&dir.0);
        state.picker = Some(PickerState::new());

        // Drive the flow: "new space..." -> name -> emoji (skip) -> folder.
        // The picker lands on the Target step with the file already saved.
        for key in [Key::Down, Key::Down, Key::Enter] {
            state.on_key(key);
        }
        for ch in "lab".chars() {
            state.on_key(Key::Char(ch));
        }
        state.on_key(Key::Enter); // name
        state.on_key(Key::Enter); // emoji skipped
        for ch in folder.0.to_string_lossy().chars() {
            state.on_key(Key::Char(ch));
        }
        state.on_key(Key::Enter); // folder -> saves spaces.json

        assert_eq!(state.list.len(), 1, "the space was saved");
        assert!(state.picker.is_some(), "picker still open on target step");
        assert!(!state.maybe_reload(), "own save must not look external");
        assert!(state.picker.is_some(), "picker survives the poll");
    }
}
