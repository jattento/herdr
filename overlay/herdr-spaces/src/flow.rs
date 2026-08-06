//! Picker state machine, as pure data.
//!
//! The caller feeds translated key presses in and gets an [`Action`] back; the
//! herdr side turns that action into workspace/worktree creation. Nothing here
//! knows about ratatui, crossterm, or herdr state.

use std::path::{Path, PathBuf};

use crate::model::{expand_home, normalize_path, Space};
use crate::store;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Pick a space, a plain workspace, or start a new space.
    Spaces,
    /// Pick one of the selected space's folders.
    Folders,
    /// Pick how to open the chosen folder.
    Target,
    NewSpaceName,
    NewSpaceEmoji,
    NewSpaceFolder,
    AddFolder,
}

impl Step {
    pub fn is_text_input(self) -> bool {
        matches!(
            self,
            Self::NewSpaceName | Self::NewSpaceEmoji | Self::NewSpaceFolder | Self::AddFolder
        )
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Spaces => "new workspace",
            Self::Folders => "space folders",
            Self::Target => "open folder as",
            Self::NewSpaceName | Self::NewSpaceEmoji | Self::NewSpaceFolder => "new space",
            Self::AddFolder => "add folder",
        }
    }

    /// Prompt shown next to the text input, for text steps only.
    pub fn prompt(self) -> &'static str {
        match self {
            Self::NewSpaceName => "name",
            Self::NewSpaceEmoji => "emoji (optional)",
            Self::NewSpaceFolder | Self::AddFolder => "folder path",
            _ => "",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Space(usize),
    PlainWorkspace,
    NewSpace,
    Folder(usize),
    AddFolder,
    Local,
    Worktree,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub kind: RowKind,
    pub label: String,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Enter,
    Esc,
    Backspace,
    Char(char),
}

/// What the herdr side should do after a key press.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    /// Close the picker and go back to the previous mode.
    Close,
    /// Fall through to herdr's own new-workspace dialog.
    PlainWorkspace,
    /// Create a workspace with this cwd.
    CreateLocal(PathBuf),
    /// Open herdr's new-linked-worktree dialog seeded with this folder.
    CreateWorktree(PathBuf),
}

/// Side effects the state machine cannot do itself: filesystem checks and
/// persistence. Real runs use [`FsEnv`]; tests use a fake.
pub trait Env {
    fn is_dir(&self, path: &Path) -> bool;
    fn save(&self, spaces: &[Space]) -> Result<(), String>;
}

/// Real environment, writing to `<config dir>/spaces.json`.
pub struct FsEnv {
    file: PathBuf,
}

impl FsEnv {
    pub fn new(file: PathBuf) -> Self {
        Self { file }
    }
}

impl Env for FsEnv {
    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn save(&self, spaces: &[Space]) -> Result<(), String> {
        store::save(&self.file, spaces)
            .map_err(|err| format!("could not save {}: {err}", store::FILE_NAME))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerState {
    step: Step,
    query: String,
    input: String,
    selected: usize,
    error: Option<String>,
    space: Option<usize>,
    folder: Option<PathBuf>,
    draft_name: String,
    draft_emoji: Option<String>,
}

impl Default for PickerState {
    fn default() -> Self {
        Self::new()
    }
}

impl PickerState {
    pub fn new() -> Self {
        Self {
            step: Step::Spaces,
            query: String::new(),
            input: String::new(),
            selected: 0,
            error: None,
            space: None,
            folder: None,
            draft_name: String::new(),
            draft_emoji: None,
        }
    }

    pub fn step(&self) -> Step {
        self.step
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Folder chosen so far, shown as context on the target step.
    pub fn folder(&self) -> Option<&Path> {
        self.folder.as_deref()
    }

    pub fn select(&mut self, row: usize) {
        self.selected = row;
    }

    /// Visible rows for the current step, already filtered by the query.
    pub fn rows(&self, spaces: &[Space]) -> Vec<Row> {
        let rows = match self.step {
            Step::Spaces => {
                let mut rows = spaces
                    .iter()
                    .enumerate()
                    .map(|(idx, space)| Row {
                        kind: RowKind::Space(idx),
                        label: space.label(),
                        detail: space.folder_count_label(),
                    })
                    .collect::<Vec<_>>();
                rows.push(Row {
                    kind: RowKind::PlainWorkspace,
                    label: "plain workspace...".into(),
                    detail: String::new(),
                });
                rows.push(Row {
                    kind: RowKind::NewSpace,
                    label: "new space...".into(),
                    detail: String::new(),
                });
                rows
            }
            Step::Folders => {
                let mut rows = self
                    .space
                    .and_then(|idx| spaces.get(idx))
                    .map(|space| {
                        space
                            .folders
                            .iter()
                            .enumerate()
                            .map(|(idx, folder)| Row {
                                kind: RowKind::Folder(idx),
                                label: folder_name(folder),
                                detail: folder.display().to_string(),
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                rows.push(Row {
                    kind: RowKind::AddFolder,
                    label: "add folder...".into(),
                    detail: String::new(),
                });
                rows
            }
            Step::Target => vec![
                Row {
                    kind: RowKind::Local,
                    label: "local".into(),
                    detail: "workspace in this folder".into(),
                },
                Row {
                    kind: RowKind::Worktree,
                    label: "worktree".into(),
                    detail: "new linked worktree from this folder".into(),
                },
            ],
            _ => Vec::new(),
        };

        let query = self.query.trim();
        if query.is_empty() {
            return rows;
        }
        rows.into_iter()
            .filter(|row| matches_query(query, &format!("{} {}", row.label, row.detail)))
            .collect()
    }

    pub fn selected_row(&self, spaces: &[Space]) -> Option<Row> {
        let rows = self.rows(spaces);
        rows.get(self.selected)
            .cloned()
            .or_else(|| rows.first().cloned())
    }

    pub fn on_key(&mut self, key: Key, spaces: &mut Vec<Space>, env: &dyn Env) -> Action {
        match key {
            Key::Up => {
                self.selected = self.selected.saturating_sub(1);
                Action::None
            }
            Key::Down => {
                let last = self.rows(spaces).len().saturating_sub(1);
                self.selected = self.selected.saturating_add(1).min(last);
                Action::None
            }
            Key::Backspace => {
                if self.step.is_text_input() {
                    self.input.pop();
                } else {
                    self.query.pop();
                    self.clamp_selection(spaces);
                }
                Action::None
            }
            Key::Char(c) => {
                if self.step.is_text_input() {
                    self.input.push(c);
                } else {
                    self.query.push(c);
                    self.selected = 0;
                }
                Action::None
            }
            Key::Esc => self.back(spaces),
            Key::Enter => self.confirm(spaces, env),
        }
    }

    /// Esc backs up one step; Esc on the first step closes the picker.
    fn back(&mut self, spaces: &[Space]) -> Action {
        self.error = None;
        match self.step {
            Step::Spaces => return Action::Close,
            Step::Folders => {
                self.space = None;
                self.enter_list_step(Step::Spaces);
            }
            Step::Target => {
                if self.space_has_folder_choice(spaces) {
                    self.enter_list_step(Step::Folders);
                } else {
                    self.space = None;
                    self.enter_list_step(Step::Spaces);
                }
                self.folder = None;
            }
            Step::NewSpaceName => {
                self.enter_list_step(Step::Spaces);
                self.input.clear();
            }
            Step::NewSpaceEmoji => {
                self.step = Step::NewSpaceName;
                self.input = self.draft_name.clone();
            }
            Step::NewSpaceFolder => {
                self.step = Step::NewSpaceEmoji;
                self.input = self.draft_emoji.clone().unwrap_or_default();
            }
            Step::AddFolder => {
                self.enter_list_step(Step::Folders);
                self.input.clear();
            }
        }
        Action::None
    }

    fn confirm(&mut self, spaces: &mut Vec<Space>, env: &dyn Env) -> Action {
        self.error = None;
        if self.step.is_text_input() {
            return self.confirm_text(spaces, env);
        }

        let Some(row) = self.selected_row(spaces) else {
            return Action::None;
        };
        match row.kind {
            RowKind::Space(idx) => {
                self.space = Some(idx);
                let folders = spaces
                    .get(idx)
                    .map(|space| space.folders.len())
                    .unwrap_or(0);
                if folders == 1 {
                    self.folder = spaces[idx].folders.first().cloned();
                    self.enter_list_step(Step::Target);
                } else {
                    self.enter_list_step(Step::Folders);
                }
                Action::None
            }
            RowKind::PlainWorkspace => Action::PlainWorkspace,
            RowKind::NewSpace => {
                self.draft_name.clear();
                self.draft_emoji = None;
                self.input.clear();
                self.step = Step::NewSpaceName;
                Action::None
            }
            RowKind::Folder(idx) => {
                self.folder = self
                    .space
                    .and_then(|space| spaces.get(space))
                    .and_then(|space| space.folders.get(idx))
                    .cloned();
                self.enter_list_step(Step::Target);
                Action::None
            }
            RowKind::AddFolder => {
                self.input.clear();
                self.step = Step::AddFolder;
                Action::None
            }
            RowKind::Local => self
                .folder
                .clone()
                .map(Action::CreateLocal)
                .unwrap_or(Action::None),
            RowKind::Worktree => self
                .folder
                .clone()
                .map(Action::CreateWorktree)
                .unwrap_or(Action::None),
        }
    }

    fn confirm_text(&mut self, spaces: &mut Vec<Space>, env: &dyn Env) -> Action {
        let value = self.input.trim().to_string();
        match self.step {
            Step::NewSpaceName => {
                if value.is_empty() {
                    self.error = Some("name is required".into());
                    return Action::None;
                }
                self.draft_name = value;
                self.input = self.draft_emoji.clone().unwrap_or_default();
                self.step = Step::NewSpaceEmoji;
            }
            Step::NewSpaceEmoji => {
                self.draft_emoji = (!value.is_empty()).then_some(value);
                self.input.clear();
                self.step = Step::NewSpaceFolder;
            }
            Step::NewSpaceFolder => {
                let folder = match self.validate_folder(&value, env) {
                    Ok(folder) => folder,
                    Err(message) => {
                        self.error = Some(message);
                        return Action::None;
                    }
                };
                let space = Space::new(
                    self.draft_name.clone(),
                    self.draft_emoji.clone(),
                    folder.clone(),
                );
                spaces.push(space);
                if let Err(message) = env.save(spaces) {
                    spaces.pop();
                    self.error = Some(message);
                    return Action::None;
                }
                self.space = Some(spaces.len() - 1);
                self.folder = Some(folder);
                self.enter_list_step(Step::Target);
            }
            Step::AddFolder => {
                let folder = match self.validate_folder(&value, env) {
                    Ok(folder) => folder,
                    Err(message) => {
                        self.error = Some(message);
                        return Action::None;
                    }
                };
                let Some(space_idx) = self.space else {
                    return Action::None;
                };
                let Some(space) = spaces.get_mut(space_idx) else {
                    return Action::None;
                };
                if !space.contains_folder(&folder) {
                    space.folders.push(folder.clone());
                    if let Err(message) = env.save(spaces) {
                        if let Some(space) = spaces.get_mut(space_idx) {
                            space.folders.pop();
                        }
                        self.error = Some(message);
                        return Action::None;
                    }
                }
                self.folder = Some(folder);
                self.enter_list_step(Step::Target);
            }
            _ => {}
        }
        Action::None
    }

    fn validate_folder(&self, value: &str, env: &dyn Env) -> Result<PathBuf, String> {
        if value.is_empty() {
            return Err("folder is required".into());
        }
        let folder = expand_home(Path::new(value));
        if !folder.is_absolute() {
            return Err("folder path must be absolute".into());
        }
        if !env.is_dir(&folder) {
            return Err(format!("not a directory: {}", folder.display()));
        }
        // Normalize only after confirming the folder exists and is absolute,
        // so the "must be absolute" / "not a directory" error messages still
        // echo back exactly what the user typed.
        Ok(normalize_path(&folder))
    }

    fn space_has_folder_choice(&self, spaces: &[Space]) -> bool {
        self.space
            .and_then(|idx| spaces.get(idx))
            .is_some_and(|space| space.folders.len() > 1)
    }

    fn enter_list_step(&mut self, step: Step) {
        self.step = step;
        self.query.clear();
        self.input.clear();
        self.selected = 0;
    }

    fn clamp_selection(&mut self, spaces: &[Space]) {
        let last = self.rows(spaces).len().saturating_sub(1);
        self.selected = self.selected.min(last);
    }
}

fn folder_name(folder: &Path) -> String {
    folder
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| folder.display().to_string())
}

/// Same matching rule herdr uses for its own modal filters: every
/// whitespace-separated needle must appear somewhere in the row text.
fn matches_query(query: &str, text: &str) -> bool {
    let haystack = text.to_lowercase();
    query
        .to_lowercase()
        .split_whitespace()
        .all(|needle| haystack.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashSet;

    struct FakeEnv {
        dirs: HashSet<PathBuf>,
        saved: RefCell<Vec<Vec<Space>>>,
        fail_save: bool,
    }

    impl FakeEnv {
        fn new(dirs: &[&str]) -> Self {
            Self {
                dirs: dirs.iter().map(PathBuf::from).collect(),
                saved: RefCell::new(Vec::new()),
                fail_save: false,
            }
        }

        fn failing(dirs: &[&str]) -> Self {
            Self {
                fail_save: true,
                ..Self::new(dirs)
            }
        }

        fn saves(&self) -> usize {
            self.saved.borrow().len()
        }
    }

    impl Env for FakeEnv {
        fn is_dir(&self, path: &Path) -> bool {
            self.dirs.contains(path)
        }

        fn save(&self, spaces: &[Space]) -> Result<(), String> {
            if self.fail_save {
                return Err("disk on fire".into());
            }
            self.saved.borrow_mut().push(spaces.to_vec());
            Ok(())
        }
    }

    fn spaces() -> Vec<Space> {
        vec![
            Space {
                id: "one".into(),
                name: "keyway".into(),
                emoji: Some("K".into()),
                folders: vec![PathBuf::from("/work/a"), PathBuf::from("/work/b")],
            },
            Space {
                id: "two".into(),
                name: "side".into(),
                emoji: None,
                folders: vec![PathBuf::from("/side")],
            },
        ]
    }

    fn type_text(picker: &mut PickerState, spaces: &mut Vec<Space>, env: &dyn Env, text: &str) {
        for c in text.chars() {
            picker.on_key(Key::Char(c), spaces, env);
        }
    }

    #[test]
    fn first_step_lists_spaces_plus_fixed_entries() {
        let picker = PickerState::new();
        let rows = picker.rows(&spaces());
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].label, "K keyway");
        assert_eq!(rows[0].detail, "2 folders");
        assert_eq!(rows[1].detail, "1 folder");
        assert_eq!(rows[2].kind, RowKind::PlainWorkspace);
        assert_eq!(rows[3].kind, RowKind::NewSpace);
    }

    #[test]
    fn typing_filters_rows_and_resets_selection() {
        let mut spaces = spaces();
        let env = FakeEnv::new(&[]);
        let mut picker = PickerState::new();
        picker.on_key(Key::Down, &mut spaces, &env);
        type_text(&mut picker, &mut spaces, &env, "side");
        assert_eq!(picker.selected(), 0);
        let rows = picker.rows(&spaces);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, RowKind::Space(1));
    }

    #[test]
    fn backspace_restores_filtered_rows() {
        let mut spaces = spaces();
        let env = FakeEnv::new(&[]);
        let mut picker = PickerState::new();
        type_text(&mut picker, &mut spaces, &env, "side");
        picker.on_key(Key::Backspace, &mut spaces, &env);
        assert_eq!(picker.query(), "sid");
        for _ in 0..3 {
            picker.on_key(Key::Backspace, &mut spaces, &env);
        }
        assert_eq!(picker.rows(&spaces).len(), 4);
    }

    #[test]
    fn multi_folder_space_shows_folder_step() {
        let mut spaces = spaces();
        let env = FakeEnv::new(&[]);
        let mut picker = PickerState::new();
        assert_eq!(picker.on_key(Key::Enter, &mut spaces, &env), Action::None);
        assert_eq!(picker.step(), Step::Folders);
        let rows = picker.rows(&spaces);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].label, "a");
        assert_eq!(rows[0].detail, "/work/a");
        assert_eq!(rows[2].kind, RowKind::AddFolder);
    }

    #[test]
    fn single_folder_space_skips_the_folder_step() {
        let mut spaces = spaces();
        let env = FakeEnv::new(&[]);
        let mut picker = PickerState::new();
        picker.on_key(Key::Down, &mut spaces, &env);
        picker.on_key(Key::Enter, &mut spaces, &env);
        assert_eq!(picker.step(), Step::Target);
        assert_eq!(picker.folder(), Some(Path::new("/side")));
    }

    #[test]
    fn local_and_worktree_targets_return_the_folder() {
        let mut spaces = spaces();
        let env = FakeEnv::new(&[]);
        let mut picker = PickerState::new();
        picker.on_key(Key::Down, &mut spaces, &env);
        picker.on_key(Key::Enter, &mut spaces, &env);
        assert_eq!(
            picker.on_key(Key::Enter, &mut spaces, &env),
            Action::CreateLocal(PathBuf::from("/side"))
        );
        picker.on_key(Key::Down, &mut spaces, &env);
        assert_eq!(
            picker.on_key(Key::Enter, &mut spaces, &env),
            Action::CreateWorktree(PathBuf::from("/side"))
        );
    }

    #[test]
    fn plain_workspace_falls_through() {
        let mut spaces = spaces();
        let env = FakeEnv::new(&[]);
        let mut picker = PickerState::new();
        type_text(&mut picker, &mut spaces, &env, "plain");
        assert_eq!(
            picker.on_key(Key::Enter, &mut spaces, &env),
            Action::PlainWorkspace
        );
    }

    #[test]
    fn esc_backtracks_one_step_at_a_time() {
        let mut spaces = spaces();
        let env = FakeEnv::new(&[]);
        let mut picker = PickerState::new();
        picker.on_key(Key::Enter, &mut spaces, &env);
        picker.on_key(Key::Enter, &mut spaces, &env);
        assert_eq!(picker.step(), Step::Target);
        picker.on_key(Key::Esc, &mut spaces, &env);
        assert_eq!(picker.step(), Step::Folders);
        picker.on_key(Key::Esc, &mut spaces, &env);
        assert_eq!(picker.step(), Step::Spaces);
        assert_eq!(picker.on_key(Key::Esc, &mut spaces, &env), Action::Close);
    }

    #[test]
    fn esc_from_target_of_single_folder_space_returns_to_spaces() {
        let mut spaces = spaces();
        let env = FakeEnv::new(&[]);
        let mut picker = PickerState::new();
        picker.on_key(Key::Down, &mut spaces, &env);
        picker.on_key(Key::Enter, &mut spaces, &env);
        picker.on_key(Key::Esc, &mut spaces, &env);
        assert_eq!(picker.step(), Step::Spaces);
        assert_eq!(picker.folder(), None);
    }

    #[test]
    fn new_space_requires_a_name() {
        let mut spaces = spaces();
        let env = FakeEnv::new(&["/work/new"]);
        let mut picker = PickerState::new();
        type_text(&mut picker, &mut spaces, &env, "new space");
        picker.on_key(Key::Enter, &mut spaces, &env);
        assert_eq!(picker.step(), Step::NewSpaceName);
        picker.on_key(Key::Enter, &mut spaces, &env);
        assert_eq!(picker.step(), Step::NewSpaceName);
        assert_eq!(picker.error(), Some("name is required"));
    }

    #[test]
    fn new_space_accepts_empty_emoji_and_validates_the_folder() {
        let mut spaces = spaces();
        let env = FakeEnv::new(&["/work/new"]);
        let mut picker = PickerState::new();
        type_text(&mut picker, &mut spaces, &env, "new space");
        picker.on_key(Key::Enter, &mut spaces, &env);
        type_text(&mut picker, &mut spaces, &env, "third");
        picker.on_key(Key::Enter, &mut spaces, &env);
        assert_eq!(picker.step(), Step::NewSpaceEmoji);
        picker.on_key(Key::Enter, &mut spaces, &env);
        assert_eq!(picker.step(), Step::NewSpaceFolder);

        type_text(&mut picker, &mut spaces, &env, "/work/missing");
        picker.on_key(Key::Enter, &mut spaces, &env);
        assert_eq!(picker.step(), Step::NewSpaceFolder);
        assert_eq!(picker.error(), Some("not a directory: /work/missing"));
        assert_eq!(spaces.len(), 2);

        for _ in 0.."/work/missing".len() {
            picker.on_key(Key::Backspace, &mut spaces, &env);
        }
        type_text(&mut picker, &mut spaces, &env, "relative/path");
        picker.on_key(Key::Enter, &mut spaces, &env);
        assert_eq!(picker.error(), Some("folder path must be absolute"));

        for _ in 0.."relative/path".len() {
            picker.on_key(Key::Backspace, &mut spaces, &env);
        }
        type_text(&mut picker, &mut spaces, &env, "/work/new");
        picker.on_key(Key::Enter, &mut spaces, &env);
        assert_eq!(picker.step(), Step::Target);
        assert_eq!(spaces.len(), 3);
        assert_eq!(spaces[2].name, "third");
        assert_eq!(spaces[2].emoji, None);
        assert_eq!(env.saves(), 1);
        assert_eq!(picker.folder(), Some(Path::new("/work/new")));
    }

    #[test]
    fn failed_save_keeps_the_space_list_unchanged() {
        let mut spaces = spaces();
        let env = FakeEnv::failing(&["/work/new"]);
        let mut picker = PickerState::new();
        type_text(&mut picker, &mut spaces, &env, "new space");
        picker.on_key(Key::Enter, &mut spaces, &env);
        type_text(&mut picker, &mut spaces, &env, "third");
        picker.on_key(Key::Enter, &mut spaces, &env);
        picker.on_key(Key::Enter, &mut spaces, &env);
        type_text(&mut picker, &mut spaces, &env, "/work/new");
        picker.on_key(Key::Enter, &mut spaces, &env);
        assert_eq!(picker.step(), Step::NewSpaceFolder);
        assert_eq!(picker.error(), Some("disk on fire"));
        assert_eq!(spaces.len(), 2);
    }

    #[test]
    fn esc_walks_back_through_the_new_space_inputs() {
        let mut spaces = spaces();
        let env = FakeEnv::new(&["/work/new"]);
        let mut picker = PickerState::new();
        type_text(&mut picker, &mut spaces, &env, "new space");
        picker.on_key(Key::Enter, &mut spaces, &env);
        type_text(&mut picker, &mut spaces, &env, "third");
        picker.on_key(Key::Enter, &mut spaces, &env);
        type_text(&mut picker, &mut spaces, &env, "X");
        picker.on_key(Key::Enter, &mut spaces, &env);
        assert_eq!(picker.step(), Step::NewSpaceFolder);
        picker.on_key(Key::Esc, &mut spaces, &env);
        assert_eq!(picker.step(), Step::NewSpaceEmoji);
        assert_eq!(picker.input(), "X");
        picker.on_key(Key::Esc, &mut spaces, &env);
        assert_eq!(picker.step(), Step::NewSpaceName);
        assert_eq!(picker.input(), "third");
        picker.on_key(Key::Esc, &mut spaces, &env);
        assert_eq!(picker.step(), Step::Spaces);
        assert_eq!(picker.query(), "");
    }

    #[test]
    fn add_folder_appends_to_the_space_and_saves() {
        let mut spaces = spaces();
        let env = FakeEnv::new(&["/work/c"]);
        let mut picker = PickerState::new();
        picker.on_key(Key::Enter, &mut spaces, &env);
        type_text(&mut picker, &mut spaces, &env, "add folder");
        picker.on_key(Key::Enter, &mut spaces, &env);
        assert_eq!(picker.step(), Step::AddFolder);
        type_text(&mut picker, &mut spaces, &env, "/work/c");
        picker.on_key(Key::Enter, &mut spaces, &env);
        assert_eq!(picker.step(), Step::Target);
        assert_eq!(spaces[0].folders.len(), 3);
        assert_eq!(env.saves(), 1);
        assert_eq!(picker.folder(), Some(Path::new("/work/c")));
    }

    #[test]
    fn add_folder_does_not_duplicate_an_existing_folder() {
        let mut spaces = spaces();
        let env = FakeEnv::new(&["/work/a"]);
        let mut picker = PickerState::new();
        picker.on_key(Key::Enter, &mut spaces, &env);
        type_text(&mut picker, &mut spaces, &env, "add folder");
        picker.on_key(Key::Enter, &mut spaces, &env);
        type_text(&mut picker, &mut spaces, &env, "/work/a");
        picker.on_key(Key::Enter, &mut spaces, &env);
        assert_eq!(spaces[0].folders.len(), 2);
        assert_eq!(env.saves(), 0);
        assert_eq!(picker.folder(), Some(Path::new("/work/a")));
    }

    #[test]
    fn selection_is_clamped_to_visible_rows() {
        let mut spaces = spaces();
        let env = FakeEnv::new(&[]);
        let mut picker = PickerState::new();
        for _ in 0..10 {
            picker.on_key(Key::Down, &mut spaces, &env);
        }
        assert_eq!(picker.selected(), 3);
        type_text(&mut picker, &mut spaces, &env, "keyway");
        assert_eq!(picker.selected(), 0);
        assert_eq!(
            picker.selected_row(&spaces).map(|row| row.kind),
            Some(RowKind::Space(0))
        );
    }
}
