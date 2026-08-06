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

pub use flow::{Action, Env, FsEnv, Key, PickerState, Row, RowKind, Step};
pub use grouping::{header_label, header_run, space_index_for_path, space_index_for_workspace};
pub use model::{Space, SpacesFile};
pub use view::{Segment, Tone};

/// Loaded spaces plus the open picker, if any. Held as one field on herdr's
/// `AppState` so the fork touches the state struct exactly once.
#[derive(Debug, Default)]
pub struct SpacesState {
    pub list: Vec<Space>,
    pub picker: Option<PickerState>,
    file: PathBuf,
}

impl SpacesState {
    /// Load `<config_dir>/spaces.json`. A missing file yields an empty list.
    pub fn load(config_dir: &Path) -> Self {
        let file = config_dir.join(store::FILE_NAME);
        Self {
            list: store::load(&file),
            picker: None,
            file,
        }
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
        picker.on_key(key, &mut self.list, &env)
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
}
