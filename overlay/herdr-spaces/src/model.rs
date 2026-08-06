//! Data model for spaces: a user-defined group of folders with a label.

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A user-defined group: a name, an optional emoji, and one or more folders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Space {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,
    #[serde(default)]
    pub folders: Vec<PathBuf>,
}

impl Space {
    pub fn new(name: impl Into<String>, emoji: Option<String>, folder: PathBuf) -> Self {
        let name = name.into();
        Self {
            id: generate_id(&name),
            emoji: emoji.filter(|value| !value.trim().is_empty()),
            name,
            folders: vec![folder],
        }
    }

    /// `"<emoji> <name>"`, or just the name when no emoji is set.
    pub fn label(&self) -> String {
        match self
            .emoji
            .as_deref()
            .map(str::trim)
            .filter(|e| !e.is_empty())
        {
            Some(emoji) => format!("{emoji} {}", self.name),
            None => self.name.clone(),
        }
    }

    pub fn folder_count_label(&self) -> String {
        match self.folders.len() {
            1 => "1 folder".to_string(),
            count => format!("{count} folders"),
        }
    }

    pub fn contains_folder(&self, folder: &Path) -> bool {
        let normalized = normalize_path(folder);
        self.folders
            .iter()
            .any(|known| normalize_path(known) == normalized)
    }
}

/// On-disk shape of `spaces.json`. Unknown fields are ignored on load.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpacesFile {
    pub version: u32,
    #[serde(default)]
    pub spaces: Vec<Space>,
}

pub const SPACES_FILE_VERSION: u32 = 1;

impl Default for SpacesFile {
    fn default() -> Self {
        Self {
            version: SPACES_FILE_VERSION,
            spaces: Vec::new(),
        }
    }
}

/// Stable-enough id: a name slug plus a millisecond timestamp. Avoids a uuid
/// dependency; collisions would need two spaces created in the same
/// millisecond with the same name.
pub fn generate_id(name: &str) -> String {
    let slug = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let slug = slug.trim_matches('-').to_string();
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or(0);
    if slug.is_empty() {
        format!("space-{millis}")
    } else {
        format!("{slug}-{millis}")
    }
}

/// Expand a leading `~` against the user's home directory.
pub fn expand_home(path: &Path) -> PathBuf {
    let Some(text) = path.to_str() else {
        return path.to_path_buf();
    };
    let Some(rest) = text.strip_prefix('~') else {
        return path.to_path_buf();
    };
    let Some(home) = home_dir() else {
        return path.to_path_buf();
    };
    let rest = rest.trim_start_matches(['/', '\\']);
    if rest.is_empty() {
        home
    } else {
        home.join(rest)
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|home| !home.as_os_str().is_empty())
}

/// Expand a leading `~`, then resolve the path to a stable, comparable form:
/// the real (canonicalized) path when it exists on disk, or a lexically
/// cleaned absolute path (no filesystem access) when it does not. Two paths
/// that name the same folder -- via a symlink, `..`, or a relative segment --
/// normalize to the same value, which is what matching and duplicate
/// detection need instead of a plain lexical comparison.
pub(crate) fn normalize_path(path: &Path) -> PathBuf {
    let expanded = expand_home(path);
    if let Ok(canonical) = std::fs::canonicalize(&expanded) {
        return canonical;
    }
    lexically_clean(&make_absolute(&expanded))
}

fn make_absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

/// Collapse `.` and `..` components without touching the filesystem, the way
/// [`std::fs::canonicalize`] would if the path existed.
fn lexically_clean(path: &Path) -> PathBuf {
    let mut out: Vec<Component> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match out.last() {
                Some(Component::Normal(_)) => {
                    out.pop();
                }
                Some(Component::RootDir | Component::Prefix(_)) => {}
                _ => out.push(component),
            },
            other => out.push(other),
        }
    }
    out.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_includes_emoji_when_present() {
        let space = Space::new("keyway", Some("K".into()), PathBuf::from("/a"));
        assert_eq!(space.label(), "K keyway");
        let space = Space::new("keyway", None, PathBuf::from("/a"));
        assert_eq!(space.label(), "keyway");
    }

    #[test]
    fn blank_emoji_is_dropped() {
        let space = Space::new("keyway", Some("   ".into()), PathBuf::from("/a"));
        assert_eq!(space.emoji, None);
        assert_eq!(space.label(), "keyway");
    }

    #[test]
    fn generated_id_slugifies_the_name() {
        let id = generate_id("Key Way!");
        assert!(id.starts_with("key-way-"), "unexpected id {id}");
    }

    #[test]
    fn expand_home_replaces_leading_tilde() {
        let home = home_dir().expect("home directory");
        assert_eq!(expand_home(Path::new("~/src")), home.join("src"));
        assert_eq!(expand_home(Path::new("~")), home);
        assert_eq!(expand_home(Path::new("/abs")), PathBuf::from("/abs"));
    }

    #[test]
    fn normalize_path_expands_tilde_when_target_is_missing() {
        let home = home_dir().expect("home directory");
        let normalized = normalize_path(Path::new("~/herdr-spaces-test-missing-dir/./sub"));
        assert_eq!(
            normalized,
            home.join("herdr-spaces-test-missing-dir").join("sub")
        );
    }

    #[test]
    fn normalize_path_falls_back_lexically_when_missing() {
        let messy = Path::new("/definitely/does/not/exist/./deep/../path");
        assert_eq!(
            normalize_path(messy),
            PathBuf::from("/definitely/does/not/exist/path")
        );
    }

    #[test]
    fn contains_folder_matches_through_dot_dot_normalization() {
        let space = Space::new("keyway", None, PathBuf::from("/abs/a"));
        assert!(space.contains_folder(Path::new("/abs/nope/../a")));
        assert!(!space.contains_folder(Path::new("/abs/b")));
    }
}
