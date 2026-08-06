//! Data model for spaces: a user-defined group of folders with a label.

use std::path::{Path, PathBuf};

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
        self.folders.iter().any(|known| known == folder)
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
}
