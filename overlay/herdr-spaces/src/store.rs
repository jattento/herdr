//! Load and save `spaces.json`.
//!
//! Mirrors herdr's session persistence: symlinks are followed manually so a
//! write through a (possibly dangling) symlink lands on the target, and the
//! payload is written to a temp file and renamed into place.

use std::path::{Path, PathBuf};

use crate::model::{expand_home, Space, SpacesFile, SPACES_FILE_VERSION};

/// File name inside herdr's config directory.
pub const FILE_NAME: &str = "spaces.json";

/// Read the spaces list. A missing file, unreadable file, or malformed JSON
/// yields an empty list: spaces are optional, and a broken file must never
/// keep herdr from starting.
pub fn load(path: &Path) -> Vec<Space> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(file) = serde_json::from_str::<SpacesFile>(&raw) else {
        return Vec::new();
    };
    file.spaces
        .into_iter()
        .map(|mut space| {
            space.folders = space.folders.iter().map(|f| expand_home(f)).collect();
            space
        })
        .filter(|space| !space.folders.is_empty())
        .collect()
}

pub fn save(path: &Path, spaces: &[Space]) -> std::io::Result<()> {
    let target = resolve_write_target(path)?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = SpacesFile {
        version: SPACES_FILE_VERSION,
        spaces: spaces.to_vec(),
    };
    let json = serde_json::to_string_pretty(&file)?;
    let tmp_path = target.with_extension("json.tmp");
    std::fs::write(&tmp_path, json)?;
    if let Err(err) = std::fs::rename(&tmp_path, &target) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(err);
    }
    Ok(())
}

fn resolve_write_target(path: &Path) -> std::io::Result<PathBuf> {
    let mut current = path.to_path_buf();
    for _ in 0..16 {
        let meta = match std::fs::symlink_metadata(&current) {
            Ok(meta) => meta,
            Err(_) => return Ok(current),
        };
        if !meta.file_type().is_symlink() {
            return Ok(current);
        }
        let link = std::fs::read_link(&current)?;
        current = if link.is_absolute() {
            link
        } else {
            current
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(link)
        };
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0);
            let dir = std::env::temp_dir().join(format!("herdr-spaces-{tag}-{nanos}"));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            Self(dir)
        }

        fn file(&self) -> PathBuf {
            self.0.join(FILE_NAME)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn missing_file_loads_as_empty() {
        let dir = TempDir::new("missing");
        assert!(load(&dir.file()).is_empty());
    }

    #[test]
    fn malformed_file_loads_as_empty() {
        let dir = TempDir::new("malformed");
        std::fs::write(dir.file(), "{ not json").expect("write");
        assert!(load(&dir.file()).is_empty());
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let dir = TempDir::new("unknown");
        std::fs::write(
            dir.file(),
            r#"{
              "version": 1,
              "future": true,
              "spaces": [
                { "id": "a", "name": "keyway", "emoji": "K", "folders": ["/abs/a"], "extra": 3 }
              ]
            }"#,
        )
        .expect("write");
        let spaces = load(&dir.file());
        assert_eq!(spaces.len(), 1);
        assert_eq!(spaces[0].name, "keyway");
        assert_eq!(spaces[0].emoji.as_deref(), Some("K"));
        assert_eq!(spaces[0].folders, vec![PathBuf::from("/abs/a")]);
    }

    #[test]
    fn folderless_spaces_are_dropped() {
        let dir = TempDir::new("folderless");
        std::fs::write(
            dir.file(),
            r#"{ "version": 1, "spaces": [ { "id": "a", "name": "empty" } ] }"#,
        )
        .expect("write");
        assert!(load(&dir.file()).is_empty());
    }

    #[test]
    fn tilde_folders_expand_on_load() {
        let dir = TempDir::new("tilde");
        std::fs::write(
            dir.file(),
            r#"{ "version": 1, "spaces": [ { "id": "a", "name": "home", "folders": ["~/src"] } ] }"#,
        )
        .expect("write");
        let spaces = load(&dir.file());
        let home = std::env::var_os("HOME").map(PathBuf::from).expect("HOME");
        assert_eq!(spaces[0].folders, vec![home.join("src")]);
    }

    #[test]
    fn save_then_load_roundtrips_and_leaves_no_temp_file() {
        let dir = TempDir::new("roundtrip");
        let spaces = vec![
            Space::new("keyway", Some("\u{1f511}".into()), PathBuf::from("/abs/a")),
            Space::new("side", None, PathBuf::from("/abs/b")),
        ];
        save(&dir.file(), &spaces).expect("save");
        assert_eq!(load(&dir.file()), spaces);
        assert!(!dir.0.join("spaces.json.tmp").exists());

        let raw = std::fs::read_to_string(dir.file()).expect("read");
        assert!(raw.contains("\"version\": 1"), "{raw}");
    }

    #[test]
    fn save_creates_missing_directories() {
        let dir = TempDir::new("nested");
        let nested = dir.0.join("a").join("b").join(FILE_NAME);
        save(&nested, &[Space::new("x", None, PathBuf::from("/abs/x"))]).expect("save");
        assert_eq!(load(&nested).len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn save_writes_through_a_dangling_symlink() {
        let dir = TempDir::new("symlink");
        let real = dir.0.join("real.json");
        std::os::unix::fs::symlink(&real, dir.file()).expect("symlink");
        save(
            &dir.file(),
            &[Space::new("x", None, PathBuf::from("/abs/x"))],
        )
        .expect("save");
        assert!(real.exists());
        assert_eq!(load(&dir.file()).len(), 1);
    }
}
