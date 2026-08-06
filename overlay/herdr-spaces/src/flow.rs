//! Picker state machine, as pure data.
//!
//! The caller feeds translated key presses in and gets an [`Action`] back; the
//! herdr side turns that action into workspace/worktree creation. Nothing here
//! knows about ratatui, crossterm, or herdr state.

use std::path::{Path, PathBuf};

use crate::discovery::{self, DirectorySuggestion, ProjectScan};
use crate::emoji;
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
        matches!(self, Self::NewSpaceName)
    }

    pub fn is_folder_picker(self) -> bool {
        matches!(self, Self::NewSpaceFolder | Self::AddFolder)
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

    pub fn prompt(self) -> &'static str {
        match self {
            Self::NewSpaceName => "name",
            Self::NewSpaceEmoji => "emoji",
            Self::NewSpaceFolder | Self::AddFolder => "folder",
            _ => "filter",
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
    NoEmoji,
    Emoji(usize),
    CustomEmoji,
    FolderChoice(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub kind: RowKind,
    pub label: String,
    pub detail: String,
    pub selectable: bool,
}

impl Row {
    fn selectable(kind: RowKind, label: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            kind,
            label: label.into(),
            detail: detail.into(),
            selectable: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Tab,
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

/// Side effects the state machine cannot do itself: bounded filesystem
/// discovery, directory checks, and persistence. Real runs use [`FsEnv`];
/// tests use a fake.
pub trait Env {
    fn is_dir(&self, path: &Path) -> bool;
    fn scan_projects(&self) -> ProjectScan;
    fn autocomplete_directories(&self, input: &str) -> Vec<DirectorySuggestion>;
    fn save(&self, spaces: &[Space]) -> Result<(), String>;
}

/// Real environment, writing to `<config dir>/spaces.json`.
pub struct FsEnv {
    file: PathBuf,
    saved: std::cell::Cell<bool>,
}

impl FsEnv {
    pub fn new(file: PathBuf) -> Self {
        Self {
            file,
            saved: std::cell::Cell::new(false),
        }
    }

    /// Whether this env performed a successful save. Used by the caller to
    /// refresh its file fingerprint so the picker's own write is not later
    /// mistaken for an external edit by `maybe_reload`.
    pub fn saved(&self) -> bool {
        self.saved.get()
    }
}

impl Env for FsEnv {
    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn scan_projects(&self) -> ProjectScan {
        discovery::scan_projects(
            &discovery::default_scan_roots(),
            discovery::ScanBudget::default(),
        )
    }

    fn autocomplete_directories(&self, input: &str) -> Vec<DirectorySuggestion> {
        discovery::autocomplete_directories(input)
    }

    fn save(&self, spaces: &[Space]) -> Result<(), String> {
        store::save(&self.file, spaces)
            .map_err(|err| format!("could not save {}: {err}", store::FILE_NAME))
            .inspect(|_| self.saved.set(true))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FolderChoice {
    path: PathBuf,
    display: String,
    already_added: bool,
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
    project_scan: Option<ProjectScan>,
    folder_choices: Vec<FolderChoice>,
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
            project_scan: None,
            folder_choices: Vec::new(),
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

    pub fn scan_was_truncated(&self) -> bool {
        self.project_scan
            .as_ref()
            .is_some_and(|scan| scan.truncated)
    }

    /// Folder chosen so far, shown as context on the target step.
    pub fn folder(&self) -> Option<&Path> {
        self.folder.as_deref()
    }

    pub fn select(&mut self, row: usize) {
        self.selected = row;
    }

    pub fn is_manual_folder_mode(&self) -> bool {
        self.step.is_folder_picker() && discovery::is_manual_path(&self.query)
    }

    pub fn hint(&self) -> &'static str {
        if self.is_manual_folder_mode() {
            "up/down ^n/^p move  tab complete  enter select  esc back"
        } else if self.step == Step::NewSpaceName {
            "enter continue  esc back"
        } else {
            "up/down ^n/^p move  enter select  esc back"
        }
    }

    pub fn primary_action_label(&self) -> &'static str {
        if self.step == Step::NewSpaceName {
            "continue"
        } else {
            "select"
        }
    }

    pub fn empty_message(&self) -> &'static str {
        if self.is_manual_folder_mode() {
            "no matching directories"
        } else if self.step.is_folder_picker() {
            if self
                .project_scan
                .as_ref()
                .is_none_or(|scan| scan.paths.is_empty())
            {
                "no projects found; type / ~ or ."
            } else {
                "no matching projects"
            }
        } else if self.step == Step::NewSpaceEmoji {
            "no matching emoji"
        } else {
            "no matches"
        }
    }

    /// Visible rows for the current step, already filtered by the query.
    pub fn rows(&self, spaces: &[Space]) -> Vec<Row> {
        match self.step {
            Step::Spaces => self.space_rows(spaces),
            Step::Folders => self.existing_folder_rows(spaces),
            Step::Target => self.target_rows(),
            Step::NewSpaceName => Vec::new(),
            Step::NewSpaceEmoji => self.emoji_rows(),
            Step::NewSpaceFolder | Step::AddFolder => self.folder_picker_rows(),
        }
    }

    pub fn selected_row(&self, spaces: &[Space]) -> Option<Row> {
        let rows = self.rows(spaces);
        rows.get(self.selected)
            .filter(|row| row.selectable)
            .cloned()
            .or_else(|| rows.into_iter().find(|row| row.selectable))
    }

    pub fn on_key(&mut self, key: Key, spaces: &mut Vec<Space>, env: &dyn Env) -> Action {
        match key {
            Key::Up => {
                self.move_selection(spaces, -1);
                Action::None
            }
            Key::Down => {
                self.move_selection(spaces, 1);
                Action::None
            }
            Key::Tab => {
                self.complete_folder(spaces, env);
                Action::None
            }
            Key::Backspace => {
                self.error = None;
                if self.step.is_text_input() {
                    self.input.pop();
                } else {
                    self.query.pop();
                    self.after_query_changed(spaces, env);
                }
                Action::None
            }
            Key::Char(c) => {
                self.error = None;
                if self.step.is_text_input() {
                    self.input.push(c);
                } else {
                    self.query.push(c);
                    self.after_query_changed(spaces, env);
                }
                Action::None
            }
            Key::Esc => self.back(spaces, env),
            Key::Enter => self.confirm(spaces, env),
        }
    }

    /// Insert pasted text into the active field. Picker inputs are intentionally
    /// single-line: only the first line is accepted and trailing whitespace is
    /// stripped so a copied path or label does not acquire an invisible suffix.
    pub fn paste(&mut self, text: &str, spaces: &[Space], env: &dyn Env) {
        let first_line = text.lines().next().unwrap_or_default().trim_end();
        if first_line.is_empty() {
            return;
        }
        self.error = None;
        let clean = first_line
            .chars()
            .filter(|character| !character.is_control())
            .collect::<String>();
        if self.step.is_text_input() {
            self.input.push_str(&clean);
        } else {
            self.query.push_str(&clean);
            self.after_query_changed(spaces, env);
        }
    }

    fn space_rows(&self, spaces: &[Space]) -> Vec<Row> {
        let mut rows = spaces
            .iter()
            .enumerate()
            .map(|(idx, space)| {
                Row::selectable(
                    RowKind::Space(idx),
                    space.label(),
                    space.folder_count_label(),
                )
            })
            .collect::<Vec<_>>();
        rows.push(Row::selectable(
            RowKind::PlainWorkspace,
            "plain workspace...",
            "",
        ));
        rows.push(Row::selectable(RowKind::NewSpace, "new space...", ""));
        filter_rows(rows, &self.query)
    }

    fn existing_folder_rows(&self, spaces: &[Space]) -> Vec<Row> {
        let mut rows = self
            .space
            .and_then(|idx| spaces.get(idx))
            .map(|space| {
                space
                    .folders
                    .iter()
                    .enumerate()
                    .map(|(idx, folder)| {
                        Row::selectable(
                            RowKind::Folder(idx),
                            folder_name(folder),
                            discovery::display_path(folder),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        rows.push(Row::selectable(RowKind::AddFolder, "add folder...", ""));
        filter_rows(rows, &self.query)
    }

    fn target_rows(&self) -> Vec<Row> {
        filter_rows(
            vec![
                Row::selectable(RowKind::Local, "local", "workspace in this folder"),
                Row::selectable(
                    RowKind::Worktree,
                    "worktree",
                    "new linked worktree from this folder",
                ),
            ],
            &self.query,
        )
    }

    fn emoji_rows(&self) -> Vec<Row> {
        let query = self.query.trim();
        let mut rows = Vec::new();
        if query.is_empty() || matches_query(query, "none no blank empty off") {
            rows.push(Row::selectable(RowKind::NoEmoji, "none", "no emoji"));
        }
        rows.extend(
            emoji::CATALOG
                .iter()
                .copied()
                .enumerate()
                .filter(|(_, entry)| entry.matches(query))
                .map(|(idx, entry)| {
                    Row::selectable(
                        RowKind::Emoji(idx),
                        format!("{}  {}", entry.glyph, entry.name),
                        "",
                    )
                }),
        );
        if rows.is_empty() && is_custom_emoji_candidate(query) {
            rows.push(Row::selectable(
                RowKind::CustomEmoji,
                format!("use '{query}'"),
                "custom",
            ));
        }
        rows
    }

    fn folder_picker_rows(&self) -> Vec<Row> {
        let rows = self
            .folder_choices
            .iter()
            .enumerate()
            .map(|(idx, choice)| Row {
                kind: RowKind::FolderChoice(idx),
                label: choice.display.clone(),
                detail: if choice.already_added {
                    "already added".to_string()
                } else if self.is_manual_folder_mode() {
                    "directory".to_string()
                } else {
                    "project".to_string()
                },
                selectable: !choice.already_added,
            })
            .collect::<Vec<_>>();
        if self.is_manual_folder_mode() {
            rows
        } else {
            filter_folder_rows(rows, &self.query)
        }
    }

    /// Esc backs up one step; Esc on the first step closes the picker.
    fn back(&mut self, spaces: &[Space], env: &dyn Env) -> Action {
        self.error = None;
        match self.step {
            Step::Spaces => return Action::Close,
            Step::Folders => {
                self.space = None;
                self.enter_list_step(Step::Spaces, spaces);
            }
            Step::Target => {
                if self.space_has_folder_choice(spaces) {
                    self.enter_list_step(Step::Folders, spaces);
                } else {
                    self.space = None;
                    self.enter_list_step(Step::Spaces, spaces);
                }
                self.folder = None;
            }
            Step::NewSpaceName => {
                self.enter_list_step(Step::Spaces, spaces);
                self.input.clear();
            }
            Step::NewSpaceEmoji => {
                self.step = Step::NewSpaceName;
                self.input = self.draft_name.clone();
                self.query.clear();
            }
            Step::NewSpaceFolder => {
                self.enter_list_step(Step::NewSpaceEmoji, spaces);
                self.query = self.draft_emoji.clone().unwrap_or_default();
                self.select_first_selectable(spaces);
            }
            Step::AddFolder => {
                self.enter_list_step(Step::Folders, spaces);
            }
        }
        if self.step.is_folder_picker() {
            self.refresh_folder_choices(spaces, env);
        }
        Action::None
    }

    fn confirm(&mut self, spaces: &mut Vec<Space>, env: &dyn Env) -> Action {
        self.error = None;
        if self.step == Step::NewSpaceName {
            return self.confirm_name(spaces);
        }

        let row = self.selected_row(spaces);
        if row.is_none() && self.step.is_folder_picker() && self.is_manual_folder_mode() {
            return self.confirm_typed_folder(spaces, env);
        }
        let Some(row) = row else {
            return Action::None;
        };
        match row.kind {
            RowKind::Space(idx) => {
                self.space = Some(idx);
                // Always show the folder list, even for a single folder:
                // it is the only place "add folder..." is reachable.
                self.enter_list_step(Step::Folders, spaces);
                Action::None
            }
            RowKind::PlainWorkspace => Action::PlainWorkspace,
            RowKind::NewSpace => {
                self.draft_name.clear();
                self.draft_emoji = None;
                self.input.clear();
                self.query.clear();
                self.step = Step::NewSpaceName;
                Action::None
            }
            RowKind::Folder(idx) => {
                self.folder = self
                    .space
                    .and_then(|space| spaces.get(space))
                    .and_then(|space| space.folders.get(idx))
                    .cloned();
                self.enter_list_step(Step::Target, spaces);
                Action::None
            }
            RowKind::AddFolder => {
                self.enter_folder_step(Step::AddFolder, spaces, env);
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
            RowKind::NoEmoji => {
                self.draft_emoji = None;
                self.enter_folder_step(Step::NewSpaceFolder, spaces, env);
                Action::None
            }
            RowKind::Emoji(idx) => {
                let Some(entry) = emoji::CATALOG.get(idx) else {
                    return Action::None;
                };
                self.draft_emoji = Some(entry.glyph.to_string());
                self.enter_folder_step(Step::NewSpaceFolder, spaces, env);
                Action::None
            }
            RowKind::CustomEmoji => {
                self.draft_emoji = Some(self.query.trim().to_string());
                self.enter_folder_step(Step::NewSpaceFolder, spaces, env);
                Action::None
            }
            RowKind::FolderChoice(idx) => {
                let Some(choice) = self.folder_choices.get(idx).cloned() else {
                    return Action::None;
                };
                if choice.already_added {
                    self.error = Some("folder already added".into());
                    return Action::None;
                }
                self.confirm_folder(choice.path, spaces, env)
            }
        }
    }

    fn confirm_name(&mut self, spaces: &[Space]) -> Action {
        let value = self.input.trim().to_string();
        if value.is_empty() {
            self.error = Some("name is required".into());
            return Action::None;
        }
        self.draft_name = value;
        self.input.clear();
        self.enter_list_step(Step::NewSpaceEmoji, spaces);
        Action::None
    }

    fn confirm_typed_folder(&mut self, spaces: &mut Vec<Space>, env: &dyn Env) -> Action {
        let value = self.query.trim().to_string();
        let folder = match self.validate_folder(&value, env) {
            Ok(folder) => folder,
            Err(message) => {
                self.error = Some(message);
                return Action::None;
            }
        };
        if self.folder_is_already_added(spaces, &folder) {
            self.error = Some("folder already added".into());
            return Action::None;
        }
        self.confirm_folder(folder, spaces, env)
    }

    fn confirm_folder(
        &mut self,
        folder: PathBuf,
        spaces: &mut Vec<Space>,
        env: &dyn Env,
    ) -> Action {
        let folder = match self.validate_folder(&folder.display().to_string(), env) {
            Ok(folder) => folder,
            Err(message) => {
                self.error = Some(message);
                return Action::None;
            }
        };
        match self.step {
            Step::NewSpaceFolder => {
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
            }
            Step::AddFolder => {
                let Some(space_idx) = self.space else {
                    return Action::None;
                };
                let Some(space) = spaces.get_mut(space_idx) else {
                    return Action::None;
                };
                if space.contains_folder(&folder) {
                    self.error = Some("folder already added".into());
                    return Action::None;
                }
                space.folders.push(folder.clone());
                if let Err(message) = env.save(spaces) {
                    if let Some(space) = spaces.get_mut(space_idx) {
                        space.folders.pop();
                    }
                    self.error = Some(message);
                    return Action::None;
                }
            }
            _ => return Action::None,
        }
        self.folder = Some(folder);
        self.enter_list_step(Step::Target, spaces);
        Action::None
    }

    fn validate_folder(&self, value: &str, env: &dyn Env) -> Result<PathBuf, String> {
        if value.is_empty() {
            return Err("folder is required".into());
        }
        let folder = normalize_path(&expand_home(Path::new(value)));
        if !env.is_dir(&folder) {
            return Err(format!("not a directory: {}", folder.display()));
        }
        Ok(folder)
    }

    fn enter_folder_step(&mut self, step: Step, spaces: &[Space], env: &dyn Env) {
        self.enter_list_step(step, spaces);
        if self.project_scan.is_none() {
            self.project_scan = Some(env.scan_projects());
        }
        self.refresh_folder_choices(spaces, env);
    }

    fn refresh_folder_choices(&mut self, spaces: &[Space], env: &dyn Env) {
        self.folder_choices = if self.is_manual_folder_mode() {
            env.autocomplete_directories(&self.query)
                .into_iter()
                .map(|suggestion| FolderChoice {
                    already_added: self.folder_is_already_added(spaces, &suggestion.path),
                    path: suggestion.path,
                    display: suggestion.display,
                })
                .collect()
        } else {
            self.project_scan
                .as_ref()
                .map(|scan| {
                    scan.paths
                        .iter()
                        .map(|path| FolderChoice {
                            already_added: self.folder_is_already_added(spaces, path),
                            path: path.clone(),
                            display: discovery::display_path(path),
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        self.select_first_selectable(spaces);
    }

    fn complete_folder(&mut self, spaces: &[Space], env: &dyn Env) {
        if !self.is_manual_folder_mode() {
            return;
        }
        let rows = self.rows(spaces);
        let Some(row) = rows
            .get(self.selected)
            .filter(|row| row.selectable)
            .or_else(|| rows.iter().find(|row| row.selectable))
        else {
            let value = self.query.trim();
            let folder = normalize_path(&expand_home(Path::new(value)));
            if env.is_dir(&folder) {
                self.query = format!("{}/", self.query.trim_end_matches(['/', '\\']));
                self.refresh_folder_choices(spaces, env);
            }
            return;
        };
        let RowKind::FolderChoice(idx) = row.kind else {
            return;
        };
        let Some(choice) = self.folder_choices.get(idx) else {
            return;
        };
        self.query = format!("{}/", choice.display.trim_end_matches(['/', '\\']));
        self.refresh_folder_choices(spaces, env);
    }

    fn after_query_changed(&mut self, spaces: &[Space], env: &dyn Env) {
        if self.step.is_folder_picker() {
            self.refresh_folder_choices(spaces, env);
        } else {
            self.select_first_selectable(spaces);
        }
    }

    fn folder_is_already_added(&self, spaces: &[Space], folder: &Path) -> bool {
        self.space
            .and_then(|idx| spaces.get(idx))
            .is_some_and(|space| space.contains_folder(folder))
    }

    fn space_has_folder_choice(&self, spaces: &[Space]) -> bool {
        self.space
            .and_then(|idx| spaces.get(idx))
            .is_some_and(|space| space.folders.len() > 1)
    }

    fn enter_list_step(&mut self, step: Step, spaces: &[Space]) {
        self.step = step;
        self.query.clear();
        self.input.clear();
        self.selected = 0;
        self.folder_choices.clear();
        self.select_first_selectable(spaces);
    }

    fn select_first_selectable(&mut self, spaces: &[Space]) {
        self.selected = self
            .rows(spaces)
            .iter()
            .position(|row| row.selectable)
            .unwrap_or(0);
    }

    fn move_selection(&mut self, spaces: &[Space], direction: isize) {
        let rows = self.rows(spaces);
        if rows.is_empty() {
            self.selected = 0;
            return;
        }
        let mut idx = self.selected.min(rows.len().saturating_sub(1));
        loop {
            let next = if direction < 0 {
                idx.checked_sub(1)
            } else {
                idx.checked_add(1).filter(|next| *next < rows.len())
            };
            let Some(next) = next else {
                return;
            };
            idx = next;
            if rows[idx].selectable {
                self.selected = idx;
                return;
            }
        }
    }
}

fn folder_name(folder: &Path) -> String {
    folder
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| folder.display().to_string())
}

fn filter_rows(rows: Vec<Row>, query: &str) -> Vec<Row> {
    let query = query.trim();
    if query.is_empty() {
        return rows;
    }
    rows.into_iter()
        .filter(|row| matches_query(query, &format!("{} {}", row.label, row.detail)))
        .collect()
}

fn filter_folder_rows(rows: Vec<Row>, query: &str) -> Vec<Row> {
    let query = query.trim();
    if query.is_empty() {
        return rows;
    }
    rows.into_iter()
        .filter(|row| matches_fuzzy_query(query, &row.label))
        .collect()
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

fn matches_fuzzy_query(query: &str, text: &str) -> bool {
    let haystack = text.to_lowercase();
    query
        .to_lowercase()
        .split_whitespace()
        .all(|needle| haystack.contains(needle) || is_subsequence(needle.chars(), haystack.chars()))
}

fn is_subsequence(
    mut needle: impl Iterator<Item = char>,
    mut haystack: impl Iterator<Item = char>,
) -> bool {
    needle.all(|wanted| haystack.any(|candidate| candidate == wanted))
}

fn is_custom_emoji_candidate(query: &str) -> bool {
    !query.is_empty() && !query.chars().any(char::is_whitespace) && query.chars().count() <= 8
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashSet;

    struct FakeEnv {
        dirs: HashSet<PathBuf>,
        projects: Vec<PathBuf>,
        saved: RefCell<Vec<Vec<Space>>>,
        fail_save: bool,
    }

    impl FakeEnv {
        fn new(dirs: &[&str]) -> Self {
            Self {
                dirs: dirs.iter().map(PathBuf::from).collect(),
                projects: dirs.iter().map(PathBuf::from).collect(),
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

        fn scan_projects(&self) -> ProjectScan {
            ProjectScan {
                paths: self.projects.clone(),
                visited_dirs: self.projects.len(),
                truncated: false,
            }
        }

        fn autocomplete_directories(&self, input: &str) -> Vec<DirectorySuggestion> {
            let mut suggestions = self
                .dirs
                .iter()
                .filter(|path| path.display().to_string().starts_with(input))
                .map(|path| DirectorySuggestion {
                    path: path.clone(),
                    display: path.display().to_string(),
                })
                .collect::<Vec<_>>();
            suggestions.sort_by(|a, b| a.display.cmp(&b.display));
            suggestions
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
        for character in text.chars() {
            picker.on_key(Key::Char(character), spaces, env);
        }
    }

    fn enter_new_space_name(picker: &mut PickerState, spaces: &mut Vec<Space>, env: &dyn Env) {
        type_text(picker, spaces, env, "new space");
        picker.on_key(Key::Enter, spaces, env);
        type_text(picker, spaces, env, "new space");
        picker.on_key(Key::Enter, spaces, env);
    }

    #[test]
    fn first_step_lists_spaces_plus_fixed_entries() {
        let picker = PickerState::new();
        let rows = picker.rows(&spaces());
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].label, "K keyway");
        assert_eq!(rows[0].detail, "2 folders");
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
        assert_eq!(picker.rows(&spaces)[0].kind, RowKind::Space(1));
    }

    #[test]
    fn multi_and_single_folder_spaces_keep_the_folder_step() {
        let mut spaces = spaces();
        let env = FakeEnv::new(&[]);
        let mut picker = PickerState::new();
        picker.on_key(Key::Enter, &mut spaces, &env);
        assert_eq!(picker.step(), Step::Folders);
        assert_eq!(picker.rows(&spaces).len(), 3);

        picker.on_key(Key::Esc, &mut spaces, &env);
        picker.on_key(Key::Down, &mut spaces, &env);
        picker.on_key(Key::Enter, &mut spaces, &env);
        assert_eq!(picker.step(), Step::Folders);
        picker.on_key(Key::Enter, &mut spaces, &env);
        assert_eq!(picker.step(), Step::Target);
        assert_eq!(picker.folder(), Some(Path::new("/side")));
    }

    #[test]
    fn local_worktree_and_plain_workspace_actions_are_preserved() {
        let mut spaces = spaces();
        let env = FakeEnv::new(&[]);
        let mut picker = PickerState::new();
        type_text(&mut picker, &mut spaces, &env, "plain");
        assert_eq!(
            picker.on_key(Key::Enter, &mut spaces, &env),
            Action::PlainWorkspace
        );

        let mut picker = PickerState::new();
        picker.on_key(Key::Down, &mut spaces, &env);
        picker.on_key(Key::Enter, &mut spaces, &env);
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
    fn emoji_filter_matches_names_aliases_and_custom_values() {
        let mut spaces = spaces();
        let env = FakeEnv::new(&["/work/new"]);
        let mut picker = PickerState::new();
        enter_new_space_name(&mut picker, &mut spaces, &env);
        type_text(&mut picker, &mut spaces, &env, "deploy");
        let rows = picker.rows(&spaces);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label, "🚀  rocket");

        for _ in 0.."deploy".len() {
            picker.on_key(Key::Backspace, &mut spaces, &env);
        }
        type_text(&mut picker, &mut spaces, &env, "🫠");
        assert_eq!(picker.rows(&spaces)[0].kind, RowKind::CustomEmoji);
        assert_eq!(picker.rows(&spaces)[0].label, "use '🫠'");
    }

    #[test]
    fn none_is_the_first_emoji_and_selection_advances_to_folder_scan() {
        let mut spaces = spaces();
        let env = FakeEnv::new(&["/work/new"]);
        let mut picker = PickerState::new();
        enter_new_space_name(&mut picker, &mut spaces, &env);
        assert_eq!(picker.rows(&spaces)[0].kind, RowKind::NoEmoji);
        picker.on_key(Key::Enter, &mut spaces, &env);
        assert_eq!(picker.step(), Step::NewSpaceFolder);
        assert_eq!(picker.rows(&spaces)[0].label, "/work/new");
    }

    #[test]
    fn scanned_folders_are_fuzzy_filterable() {
        let mut spaces = spaces();
        let env = FakeEnv::new(&["/work/alpha-project", "/work/beta-service"]);
        let mut picker = PickerState::new();
        enter_new_space_name(&mut picker, &mut spaces, &env);
        picker.on_key(Key::Enter, &mut spaces, &env);
        type_text(&mut picker, &mut spaces, &env, "btasvc");
        let rows = picker.rows(&spaces);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label, "/work/beta-service");
    }

    #[test]
    fn already_added_folder_is_dimmed_and_not_selectable() {
        let mut spaces = spaces();
        let env = FakeEnv::new(&["/work/a", "/work/c"]);
        let mut picker = PickerState::new();
        picker.on_key(Key::Enter, &mut spaces, &env);
        type_text(&mut picker, &mut spaces, &env, "add folder");
        picker.on_key(Key::Enter, &mut spaces, &env);
        let rows = picker.rows(&spaces);
        assert!(!rows[0].selectable);
        assert_eq!(rows[0].detail, "already added");
        assert_eq!(picker.selected(), 1);
        picker.on_key(Key::Up, &mut spaces, &env);
        assert_eq!(picker.selected(), 1);
    }

    #[test]
    fn manual_path_tab_completes_and_enter_selects() {
        let mut spaces = spaces();
        let env = FakeEnv::new(&["/work/new", "/work/new/deeper"]);
        let mut picker = PickerState::new();
        enter_new_space_name(&mut picker, &mut spaces, &env);
        picker.on_key(Key::Enter, &mut spaces, &env);
        type_text(&mut picker, &mut spaces, &env, "/work/n");
        assert!(picker.is_manual_folder_mode());
        picker.on_key(Key::Tab, &mut spaces, &env);
        assert_eq!(picker.query(), "/work/new/");
        assert_eq!(picker.rows(&spaces)[0].label, "/work/new/deeper");
        picker.on_key(Key::Enter, &mut spaces, &env);
        assert_eq!(picker.step(), Step::Target);
        assert_eq!(picker.folder(), Some(Path::new("/work/new/deeper")));
    }

    #[test]
    fn non_existent_manual_path_stays_in_step_with_error() {
        let mut spaces = spaces();
        let env = FakeEnv::new(&[]);
        let mut picker = PickerState::new();
        enter_new_space_name(&mut picker, &mut spaces, &env);
        picker.on_key(Key::Enter, &mut spaces, &env);
        type_text(&mut picker, &mut spaces, &env, "/missing");
        picker.on_key(Key::Enter, &mut spaces, &env);
        assert_eq!(picker.step(), Step::NewSpaceFolder);
        assert_eq!(picker.error(), Some("not a directory: /missing"));
    }

    #[test]
    fn relative_manual_path_is_canonicalized_before_selection() {
        let cwd = std::env::current_dir().expect("cwd");
        let relative = PathBuf::from("./overlay");
        let canonical = normalize_path(&relative);
        let env = FakeEnv {
            dirs: HashSet::from([canonical.clone()]),
            projects: Vec::new(),
            saved: RefCell::new(Vec::new()),
            fail_save: false,
        };
        let mut spaces = spaces();
        let mut picker = PickerState::new();
        enter_new_space_name(&mut picker, &mut spaces, &env);
        picker.on_key(Key::Enter, &mut spaces, &env);
        type_text(&mut picker, &mut spaces, &env, "./overlay");
        picker.on_key(Key::Enter, &mut spaces, &env);
        assert_eq!(picker.folder(), Some(canonical.as_path()));
        assert!(picker
            .folder()
            .is_some_and(|folder| folder.starts_with(cwd)));
    }

    #[test]
    fn new_space_saves_selected_emoji_and_folder() {
        let mut spaces = spaces();
        let env = FakeEnv::new(&["/work/new"]);
        let mut picker = PickerState::new();
        enter_new_space_name(&mut picker, &mut spaces, &env);
        type_text(&mut picker, &mut spaces, &env, "rocket");
        picker.on_key(Key::Enter, &mut spaces, &env);
        picker.on_key(Key::Enter, &mut spaces, &env);
        assert_eq!(picker.step(), Step::Target);
        assert_eq!(spaces.len(), 3);
        assert_eq!(spaces[2].name, "new space");
        assert_eq!(spaces[2].emoji.as_deref(), Some("🚀"));
        assert_eq!(spaces[2].folders, vec![PathBuf::from("/work/new")]);
        assert_eq!(env.saves(), 1);
    }

    #[test]
    fn add_folder_appends_and_failed_save_rolls_back() {
        let mut spaces = spaces();
        let env = FakeEnv::new(&["/work/c"]);
        let mut picker = PickerState::new();
        picker.on_key(Key::Enter, &mut spaces, &env);
        type_text(&mut picker, &mut spaces, &env, "add folder");
        picker.on_key(Key::Enter, &mut spaces, &env);
        picker.on_key(Key::Enter, &mut spaces, &env);
        assert_eq!(spaces[0].folders.len(), 3);
        assert_eq!(env.saves(), 1);

        let env = FakeEnv::failing(&["/work/d"]);
        let mut picker = PickerState::new();
        picker.on_key(Key::Enter, &mut spaces, &env);
        type_text(&mut picker, &mut spaces, &env, "add folder");
        picker.on_key(Key::Enter, &mut spaces, &env);
        type_text(&mut picker, &mut spaces, &env, "/work/d");
        picker.on_key(Key::Enter, &mut spaces, &env);
        assert_eq!(spaces[0].folders.len(), 3);
        assert_eq!(picker.error(), Some("disk on fire"));
    }

    #[test]
    fn paste_uses_first_line_in_every_picker_text_field() {
        let env = FakeEnv::new(&["/work/new"]);
        let mut spaces = spaces();
        let mut picker = PickerState::new();

        picker.paste("side  \nignored", &spaces, &env);
        assert_eq!(picker.query(), "side");
        picker.on_key(Key::Backspace, &mut spaces, &env);
        picker.on_key(Key::Backspace, &mut spaces, &env);
        picker.on_key(Key::Backspace, &mut spaces, &env);
        picker.on_key(Key::Backspace, &mut spaces, &env);
        type_text(&mut picker, &mut spaces, &env, "new space");
        picker.on_key(Key::Enter, &mut spaces, &env);

        picker.paste("pasted name  \nignored", &spaces, &env);
        assert_eq!(picker.input(), "pasted name");
        picker.on_key(Key::Enter, &mut spaces, &env);

        picker.paste("rocket  \nignored", &spaces, &env);
        assert_eq!(picker.query(), "rocket");
        picker.on_key(Key::Enter, &mut spaces, &env);

        picker.paste("/work/n  \nignored", &spaces, &env);
        assert_eq!(picker.query(), "/work/n");
        assert_eq!(picker.rows(&spaces)[0].label, "/work/new");
    }

    #[test]
    fn esc_backtracks_through_new_space_steps() {
        let mut spaces = spaces();
        let env = FakeEnv::new(&["/work/new"]);
        let mut picker = PickerState::new();
        enter_new_space_name(&mut picker, &mut spaces, &env);
        type_text(&mut picker, &mut spaces, &env, "rocket");
        picker.on_key(Key::Enter, &mut spaces, &env);
        picker.on_key(Key::Esc, &mut spaces, &env);
        assert_eq!(picker.step(), Step::NewSpaceEmoji);
        assert_eq!(picker.query(), "🚀");
        picker.on_key(Key::Esc, &mut spaces, &env);
        assert_eq!(picker.step(), Step::NewSpaceName);
        assert_eq!(picker.input(), "new space");
        picker.on_key(Key::Esc, &mut spaces, &env);
        assert_eq!(picker.step(), Step::Spaces);
    }
}
