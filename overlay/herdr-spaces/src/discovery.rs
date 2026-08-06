//! Bounded project discovery and manual directory completion.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::model::{expand_home, normalize_path};

pub const MAX_SCAN_DIRS: usize = 20_000;
pub const MAX_SCAN_DURATION: Duration = Duration::from_millis(500);
pub const MAX_AUTOCOMPLETE_ENTRIES: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectScan {
    pub paths: Vec<PathBuf>,
    pub visited_dirs: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectorySuggestion {
    pub path: PathBuf,
    pub display: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanRoot {
    pub path: PathBuf,
    pub max_depth: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct ScanBudget {
    pub max_dirs: usize,
    pub max_duration: Duration,
}

impl Default for ScanBudget {
    fn default() -> Self {
        Self {
            max_dirs: MAX_SCAN_DIRS,
            max_duration: MAX_SCAN_DURATION,
        }
    }
}

pub fn default_scan_roots() -> Vec<ScanRoot> {
    let Some(home) = home_dir() else {
        return Vec::new();
    };
    let projects = home.join("projects");
    let mut roots = Vec::with_capacity(2);
    if projects.is_dir() {
        roots.push(ScanRoot {
            path: projects,
            max_depth: 4,
        });
    }
    roots.push(ScanRoot {
        path: home,
        max_depth: 1,
    });
    roots
}

pub fn scan_projects(roots: &[ScanRoot], budget: ScanBudget) -> ProjectScan {
    let started = Instant::now();
    let mut queue = VecDeque::new();
    let mut seen = HashSet::new();
    let mut paths = Vec::new();
    let mut visited_dirs = 0;
    let mut truncated = false;

    for root in roots {
        if root.path.is_dir() {
            queue.push_back((root.path.clone(), 0, root.max_depth));
        }
    }

    while let Some((dir, depth, max_depth)) = queue.pop_front() {
        if visited_dirs >= budget.max_dirs || started.elapsed() >= budget.max_duration {
            truncated = true;
            break;
        }
        let normalized = normalize_path(&dir);
        if !seen.insert(normalized.clone()) {
            continue;
        }
        visited_dirs += 1;

        if normalized.join(".git").exists() {
            paths.push(normalized.clone());
        }
        if depth >= max_depth {
            continue;
        }

        let Ok(entries) = std::fs::read_dir(&normalized) else {
            continue;
        };
        for entry in entries.flatten() {
            if started.elapsed() >= budget.max_duration {
                truncated = true;
                break;
            }
            if queue.len().saturating_add(visited_dirs) >= budget.max_dirs {
                truncated = true;
                break;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if should_skip_dir(&name) {
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                queue.push_back((entry.path(), depth + 1, max_depth));
            }
        }
        if truncated {
            break;
        }
    }

    paths.sort_by_key(|path| display_path(path));
    paths.dedup();
    ProjectScan {
        paths,
        visited_dirs,
        truncated,
    }
}

pub fn autocomplete_directories(input: &str) -> Vec<DirectorySuggestion> {
    autocomplete_directories_with(input, home_dir().as_deref(), current_dir().as_deref())
}

fn autocomplete_directories_with(
    input: &str,
    home: Option<&Path>,
    cwd: Option<&Path>,
) -> Vec<DirectorySuggestion> {
    let Some((parent, display_parent, prefix)) = completion_parts(input, home, cwd) else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&parent) else {
        return Vec::new();
    };

    let mut suggestions = Vec::new();
    for entry in entries.flatten().take(MAX_AUTOCOMPLETE_ENTRIES) {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.to_lowercase().starts_with(&prefix.to_lowercase()) {
            continue;
        }
        if name.starts_with('.') && !prefix.starts_with('.') {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let display = join_display_path(&display_parent, &name);
        suggestions.push(DirectorySuggestion {
            path: normalize_path(&entry.path()),
            display,
        });
    }
    suggestions.sort_by(|a, b| a.display.cmp(&b.display));
    suggestions
}

pub fn display_path(path: &Path) -> String {
    let normalized = normalize_path(path);
    if let Some(home) = home_dir().map(|home| std::fs::canonicalize(&home).unwrap_or(home)) {
        if normalized == home {
            return "~".to_string();
        }
        if let Ok(relative) = normalized.strip_prefix(&home) {
            return format!("~/{}", relative.display());
        }
    }
    normalized.display().to_string()
}

pub fn is_manual_path(input: &str) -> bool {
    matches!(input.chars().next(), Some('/' | '~' | '.'))
}

fn completion_parts(
    input: &str,
    home: Option<&Path>,
    cwd: Option<&Path>,
) -> Option<(PathBuf, String, String)> {
    if !is_manual_path(input) {
        return None;
    }
    let cwd = cwd?;
    if input == "~" {
        return Some((home?.to_path_buf(), "~".to_string(), String::new()));
    }
    if input == "." {
        return Some((cwd.to_path_buf(), ".".to_string(), String::new()));
    }

    let trailing_separator = input.ends_with(['/', '\\']);
    let expanded = expand_home(Path::new(input));
    let absolute = if expanded.is_absolute() {
        expanded
    } else {
        cwd.join(expanded)
    };
    let (parent, prefix) = if trailing_separator {
        (absolute, String::new())
    } else {
        (
            absolute.parent()?.to_path_buf(),
            absolute
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string(),
        )
    };
    let display_parent = if trailing_separator {
        let trimmed = input.trim_end_matches(['/', '\\']);
        if trimmed.is_empty() && input.starts_with('/') {
            "/".to_string()
        } else {
            trimmed.to_string()
        }
    } else {
        Path::new(input)
            .parent()
            .map(|path| path.display().to_string())
            .unwrap_or_default()
    };
    Some((parent, display_parent, prefix))
}

fn join_display_path(parent: &str, name: &str) -> String {
    match parent {
        "" => name.to_string(),
        "/" => format!("/{name}"),
        _ => format!("{}/{name}", parent.trim_end_matches(['/', '\\'])),
    }
}

fn should_skip_dir(name: &str) -> bool {
    name.starts_with('.')
        || matches!(
            name.to_ascii_lowercase().as_str(),
            "node_modules" | "target" | "venv" | "venvs" | "virtualenv" | "env"
        )
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

fn current_dir() -> Option<PathBuf> {
    std::env::current_dir().ok()
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
            let path = std::env::temp_dir().join(format!("herdr-spaces-discovery-{tag}-{nanos}"));
            std::fs::create_dir_all(&path).expect("create temp dir");
            Self(path)
        }

        fn mkdir(&self, relative: &str) -> PathBuf {
            let path = self.0.join(relative);
            std::fs::create_dir_all(&path).expect("create nested dir");
            path
        }

        fn git_dir(&self, relative: &str) -> PathBuf {
            let path = self.mkdir(relative);
            std::fs::create_dir_all(path.join(".git")).expect("create git marker");
            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn scanner_skips_noise_and_respects_depth_and_directory_caps() {
        let root = TempDir::new("scan");
        let keep = root.git_dir("projects/keep");
        root.git_dir("projects/node_modules/ignored");
        root.git_dir("projects/target/ignored");
        root.git_dir("projects/.hidden/ignored");
        root.git_dir("projects/a/b/c/too-deep");
        let roots = [ScanRoot {
            path: root.0.join("projects"),
            max_depth: 3,
        }];

        let scan = scan_projects(
            &roots,
            ScanBudget {
                max_dirs: 100,
                max_duration: Duration::from_secs(1),
            },
        );
        assert_eq!(scan.paths, vec![normalize_path(&keep)]);
        assert!(!scan.truncated);

        let capped = scan_projects(
            &roots,
            ScanBudget {
                max_dirs: 1,
                max_duration: Duration::from_secs(1),
            },
        );
        assert!(capped.truncated);
        assert_eq!(capped.visited_dirs, 1);

        let timed = scan_projects(
            &roots,
            ScanBudget {
                max_dirs: 100,
                max_duration: Duration::ZERO,
            },
        );
        assert!(timed.truncated);
        assert_eq!(timed.visited_dirs, 0);
    }

    #[test]
    fn autocomplete_lists_matching_subdirectories() {
        let root = TempDir::new("complete");
        root.mkdir("projects");
        root.mkdir("prototype");
        root.mkdir("notes");
        std::fs::write(root.0.join("profile.txt"), "not a dir").expect("write file");

        let input = format!("{}/pro", root.0.display());
        let matches = autocomplete_directories_with(&input, None, Some(&root.0));
        let labels = matches
            .into_iter()
            .map(|entry| entry.display)
            .collect::<Vec<_>>();
        assert_eq!(
            labels,
            vec![
                format!("{}/projects", root.0.display()),
                format!("{}/prototype", root.0.display())
            ]
        );
    }

    #[test]
    fn root_and_relative_completion_preserve_the_typed_path_style() {
        let root = TempDir::new("path-style");
        root.mkdir("projects");

        let relative = autocomplete_directories_with("./pro", None, Some(&root.0));
        assert_eq!(relative[0].display, "./projects");

        let absolute = autocomplete_directories_with("/", None, Some(&root.0));
        assert!(absolute.iter().all(|entry| entry.display.starts_with('/')));
    }
}
