//! Pure grouping helpers: which space owns a path, and how a run of grouped
//! rows is labelled. No herdr types, no rendering.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::model::{normalize_path, Space};

/// Index of the first space (in file order) that owns `path`: the path is one
/// of the space folders or lives inside one. `None` when nothing matches.
pub fn space_index_for_path(spaces: &[Space], path: &Path) -> Option<usize> {
    let normalized = normalized_workspace_path(path);
    spaces
        .iter()
        .position(|space| space.folders.iter().any(|folder| owns(folder, &normalized)))
}

/// Same as [`space_index_for_path`] but tries the worktree repo root first and
/// falls back to the workspace identity cwd, which is what the sidebar has.
pub fn space_index_for_workspace(
    spaces: &[Space],
    repo_root: Option<&Path>,
    identity_cwd: &Path,
) -> Option<usize> {
    repo_root
        .and_then(|root| space_index_for_path(spaces, root))
        .or_else(|| space_index_for_path(spaces, identity_cwd))
}

/// Component-wise containment: `/a/b` owns `/a/b` and `/a/b/c`, never `/a/bc`.
/// Both sides are expected to already be normalized: `folder` by
/// [`normalize_path`] at space load/add time, `path` by
/// [`normalized_workspace_path`] just below.
fn owns(folder: &Path, path: &Path) -> bool {
    !folder.as_os_str().is_empty() && path.starts_with(folder)
}

thread_local! {
    /// `space_index_for_path` runs once per sidebar row on every render
    /// frame, but `std::fs::canonicalize` is a syscall. Memoize each raw
    /// workspace path's normalized form so repeated frames for the same
    /// workspace never re-hit the filesystem. The render loop stays on one
    /// thread for the life of the process, so in practice this cache is
    /// warmed once per workspace, not rebuilt every frame.
    static WORKSPACE_PATH_CACHE: RefCell<HashMap<PathBuf, PathBuf>> =
        RefCell::new(HashMap::new());
}

fn normalized_workspace_path(path: &Path) -> PathBuf {
    WORKSPACE_PATH_CACHE.with(|cache| {
        if let Some(hit) = cache.borrow().get(path) {
            return hit.clone();
        }
        let normalized = normalize_path(path);
        cache
            .borrow_mut()
            .insert(path.to_path_buf(), normalized.clone());
        normalized
    })
}

/// Sidebar header text for a space, with the number of rows under it.
pub fn header_label(space: &Space, row_count: usize) -> (String, String) {
    (space.label(), format!("{row_count}"))
}

/// Run detection for a list of sidebar rows: given the space index of each row,
/// report `(space index, run length)` when `row` starts a run, else `None`.
/// Rows are looked up lazily because the caller resolves them from its own
/// state.
pub fn header_run(
    row_count: usize,
    row: usize,
    space_of: impl Fn(usize) -> Option<usize>,
) -> Option<(usize, usize)> {
    let space_idx = space_of(row)?;
    if row.checked_sub(1).and_then(&space_of) == Some(space_idx) {
        return None;
    }
    let run = (row..row_count)
        .take_while(|idx| space_of(*idx) == Some(space_idx))
        .count();
    Some((space_idx, run))
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
            let dir = std::env::temp_dir().join(format!("herdr-spaces-grouping-{tag}-{nanos}"));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
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
                folders: vec![PathBuf::from("/work")],
            },
        ]
    }

    #[test]
    fn exact_folder_matches() {
        assert_eq!(
            space_index_for_path(&spaces(), Path::new("/work/a")),
            Some(0)
        );
    }

    #[test]
    fn descendant_matches() {
        assert_eq!(
            space_index_for_path(&spaces(), Path::new("/work/b/deep/nested")),
            Some(0)
        );
    }

    #[test]
    fn first_space_in_file_order_wins() {
        // "/work" also owns "/work/a", but the first space is listed first.
        assert_eq!(
            space_index_for_path(&spaces(), Path::new("/work/a")),
            Some(0)
        );
        assert_eq!(
            space_index_for_path(&spaces(), Path::new("/work/other")),
            Some(1)
        );
    }

    #[test]
    fn sibling_prefix_does_not_match() {
        // "/work/ab" is not inside "/work/a"; only the "/work" space owns it.
        let only_a = &spaces()[..1];
        assert_eq!(space_index_for_path(only_a, Path::new("/work/ab")), None);
        assert_eq!(
            space_index_for_path(&spaces(), Path::new("/work/ab")),
            Some(1)
        );
    }

    #[test]
    fn unrelated_path_has_no_space() {
        assert_eq!(space_index_for_path(&spaces(), Path::new("/tmp/x")), None);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_folder_groups_correctly() {
        let base = TempDir::new("symlink-group");
        let real_target = base.0.join("real-project");
        std::fs::create_dir_all(&real_target).expect("create real target");
        let link = base.0.join("link-project");
        std::os::unix::fs::symlink(&real_target, &link).expect("create symlink");

        // The space folder was added as the symlink (as `normalize_path`
        // would store it at add/load time); the query is a workspace path
        // reached through the real, canonical directory.
        let canonical_target = std::fs::canonicalize(&real_target).expect("canonicalize target");
        let spaces = vec![Space {
            id: "one".into(),
            name: "via-symlink".into(),
            emoji: None,
            folders: vec![normalize_path(&link)],
        }];

        assert_eq!(space_index_for_path(&spaces, &canonical_target), Some(0));
        assert_eq!(
            space_index_for_path(&spaces, &canonical_target.join("nested")),
            Some(0)
        );
        // The reverse direction also matches: querying through the symlink
        // itself still resolves to the same normalized target.
        assert_eq!(space_index_for_path(&spaces, &link), Some(0));
    }

    #[test]
    fn repo_root_wins_over_identity_cwd() {
        let spaces = spaces();
        assert_eq!(
            space_index_for_workspace(
                &spaces,
                Some(Path::new("/work/a")),
                Path::new("/somewhere/else")
            ),
            Some(0)
        );
        assert_eq!(
            space_index_for_workspace(&spaces, None, Path::new("/work/b")),
            Some(0)
        );
        assert_eq!(
            space_index_for_workspace(
                &spaces,
                Some(Path::new("/nowhere")),
                Path::new("/work/other")
            ),
            Some(1)
        );
    }

    #[test]
    fn header_label_reports_emoji_and_count() {
        let spaces = spaces();
        assert_eq!(
            header_label(&spaces[0], 3),
            ("K keyway".to_string(), "3".to_string())
        );
        assert_eq!(
            header_label(&spaces[1], 1),
            ("side".to_string(), "1".to_string())
        );
    }

    #[test]
    fn header_run_starts_on_each_change_of_space() {
        let rows = [Some(0), Some(0), None, Some(1), Some(0)];
        let space_of = |idx: usize| rows[idx];
        assert_eq!(header_run(rows.len(), 0, space_of), Some((0, 2)));
        assert_eq!(header_run(rows.len(), 1, space_of), None);
        assert_eq!(header_run(rows.len(), 2, space_of), None);
        assert_eq!(header_run(rows.len(), 3, space_of), Some((1, 1)));
        assert_eq!(header_run(rows.len(), 4, space_of), Some((0, 1)));
    }
}
