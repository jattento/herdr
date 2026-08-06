//! Presentation data for the picker modal.
//!
//! The lines are composed here, already padded to the modal width, so the
//! herdr side only maps a tone to a style and draws. Keeps the ratatui code at
//! the call site down to a loop.

use unicode_width::UnicodeWidthStr;

use crate::flow::PickerState;
use crate::model::Space;

/// Style role of a segment. The caller maps these onto its palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Accent,
    Text,
    Dim,
    Separator,
    Row,
    RowSelected,
    Detail,
    DetailSelected,
    Disabled,
    Error,
}

pub type Segment = (String, Tone);

/// Modal body under the title: filter or prompt line, separator, then rows.
pub fn body_lines(
    picker: &PickerState,
    spaces: &[Space],
    width: u16,
    max_rows: usize,
) -> Vec<Vec<Segment>> {
    let width = usize::from(width);
    let mut lines = vec![input_line(picker), separator(width)];
    // The error line eats one row of the list viewport.
    let max_rows = match picker.error() {
        Some(error) => {
            lines.push(vec![(format!(" {error}"), Tone::Error)]);
            max_rows.saturating_sub(1)
        }
        None => max_rows,
    };

    let rows = picker.rows(spaces);
    if rows.is_empty() {
        if !picker.step().is_text_input() {
            lines.push(vec![(format!(" {}", picker.empty_message()), Tone::Dim)]);
        }
        lines.push(hint_line(picker));
        return lines;
    }

    let selected = picker.selected().min(rows.len().saturating_sub(1));
    let start = selected
        .saturating_sub(max_rows / 2)
        .min(rows.len().saturating_sub(max_rows));
    for (idx, row) in rows.iter().enumerate().skip(start).take(max_rows) {
        let highlighted = row.selectable && idx == selected;
        let (label_tone, detail_tone) = if !row.selectable {
            (Tone::Disabled, Tone::Disabled)
        } else if highlighted {
            (Tone::RowSelected, Tone::DetailSelected)
        } else {
            (Tone::Row, Tone::Detail)
        };
        let marker = if highlighted { "\u{203a}" } else { " " };
        let detail = truncate(&row.detail, width / 3);
        let detail_gap = usize::from(!detail.is_empty());
        let label_width = width
            .saturating_sub(detail.width())
            .saturating_sub(detail_gap);
        let label = if matches!(row.kind, crate::flow::RowKind::FolderChoice(_)) {
            format!(
                "{marker} {}",
                truncate_left(&row.label, label_width.saturating_sub(2))
            )
        } else {
            truncate(&format!("{marker} {}", row.label), label_width)
        };
        let pad = width
            .saturating_sub(label.width())
            .saturating_sub(detail.width());
        lines.push(vec![
            (label, label_tone),
            (" ".repeat(pad), label_tone),
            (detail, detail_tone),
        ]);
    }
    lines.push(hint_line(picker));
    lines
}

fn input_line(picker: &PickerState) -> Vec<Segment> {
    if picker.step().is_text_input() {
        return vec![
            (format!(" {} ", picker.step().prompt()), Tone::Dim),
            (format!("{}\u{2588}", picker.input()), Tone::Text),
        ];
    }

    let (prefix, placeholder) = if picker.step() == crate::flow::Step::NewSpaceEmoji {
        (" emoji ", "filter")
    } else if picker.step().is_folder_picker() {
        (" folder ", "find project or type / ~ .")
    } else {
        (" / ", "filter")
    };
    let mut spans = vec![(prefix.to_string(), Tone::Dim)];
    if picker.query().is_empty() {
        spans.push((placeholder.to_string(), Tone::Dim));
    } else {
        spans.push((format!("{}\u{2588}", picker.query()), Tone::Text));
    }
    if let Some(folder) = picker.folder() {
        spans.push((format!("  {}", folder.display()), Tone::Dim));
    }
    if picker.scan_was_truncated() && picker.step().is_folder_picker() {
        spans.push(("  scan limited".to_string(), Tone::Dim));
    }
    spans
}

fn hint_line(picker: &PickerState) -> Vec<Segment> {
    vec![(format!(" {}", picker.hint()), Tone::Dim)]
}

fn separator(width: usize) -> Vec<Segment> {
    vec![("\u{2500}".repeat(width), Tone::Separator)]
}

/// Sidebar group header: accent label on the left, dim row count on the right.
pub fn header_segments(label: &str, count: &str, width: u16) -> Vec<Segment> {
    let width = usize::from(width);
    let label = truncate(&format!(" {label}"), width);
    let count = truncate(count, width.saturating_sub(label.width()));
    let pad = width
        .saturating_sub(label.width())
        .saturating_sub(count.width())
        .saturating_sub(1);
    vec![
        (label, Tone::Accent),
        (" ".repeat(pad), Tone::Dim),
        (count, Tone::Dim),
    ]
}

fn truncate(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_string();
    }
    let mut out = String::new();
    for c in text.chars() {
        if out.width() + c.to_string().width() > width {
            break;
        }
        out.push(c);
    }
    out
}

fn truncate_left(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_string();
    }
    if width <= 1 {
        return "\u{2026}".chars().take(width).collect();
    }
    let mut suffix = String::new();
    for character in text.chars().rev() {
        if suffix.width() + character.to_string().width() + 1 > width {
            break;
        }
        suffix.insert(0, character);
    }
    format!("\u{2026}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::Key;
    use std::path::PathBuf;

    fn spaces() -> Vec<Space> {
        vec![Space {
            id: "one".into(),
            name: "keyway".into(),
            emoji: Some("\u{1f511}".into()),
            folders: vec![PathBuf::from("/work/a")],
        }]
    }

    #[test]
    fn rows_are_padded_to_the_modal_width() {
        let picker = PickerState::new();
        let lines = body_lines(&picker, &spaces(), 40, 8);
        // filter line, separator, then one row per entry.
        assert_eq!(lines.len(), 6);
        let row = &lines[2];
        let rendered = row
            .iter()
            .map(|(text, _)| text.as_str())
            .collect::<String>();
        assert_eq!(rendered.width(), 40);
        assert!(
            rendered.starts_with("\u{203a} \u{1f511} keyway"),
            "{rendered}"
        );
        assert!(rendered.ends_with("1 folder"), "{rendered}");
    }

    #[test]
    fn only_the_selected_row_is_highlighted() {
        let picker = PickerState::new();
        let lines = body_lines(&picker, &spaces(), 40, 8);
        assert_eq!(lines[2][0].1, Tone::RowSelected);
        assert_eq!(lines[3][0].1, Tone::Row);
    }

    #[test]
    fn the_visible_window_follows_the_selection() {
        let mut spaces = spaces();
        let mut picker = PickerState::new();
        let env = crate::flow::FsEnv::new(PathBuf::from("/nonexistent/spaces.json"));
        for _ in 0..2 {
            picker.on_key(Key::Down, &mut spaces, &env);
        }
        let lines = body_lines(&picker, &spaces, 40, 1);
        assert_eq!(lines.len(), 4);
        let rendered = lines[2]
            .iter()
            .map(|(text, _)| text.as_str())
            .collect::<String>();
        assert!(rendered.contains("new space..."), "{rendered}");
    }

    #[test]
    fn text_steps_show_a_prompt_and_cursor() {
        let mut spaces = spaces();
        let mut picker = PickerState::new();
        let env = crate::flow::FsEnv::new(PathBuf::from("/nonexistent/spaces.json"));
        picker.on_key(Key::Down, &mut spaces, &env);
        picker.on_key(Key::Down, &mut spaces, &env);
        picker.on_key(Key::Enter, &mut spaces, &env);
        picker.on_key(Key::Char('x'), &mut spaces, &env);
        let lines = body_lines(&picker, &spaces, 40, 8);
        assert_eq!(lines[0][0], (" name ".to_string(), Tone::Dim));
        assert_eq!(lines[0][1], ("x\u{2588}".to_string(), Tone::Text));
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn a_query_with_no_matches_says_so() {
        let mut spaces = spaces();
        let mut picker = PickerState::new();
        let env = crate::flow::FsEnv::new(PathBuf::from("/nonexistent/spaces.json"));
        for c in "zzz".chars() {
            picker.on_key(Key::Char(c), &mut spaces, &env);
        }
        let lines = body_lines(&picker, &spaces, 40, 8);
        assert_eq!(lines[2][0].0, " no matches");
        assert!(lines[3][0].0.contains("enter select"));
    }

    #[test]
    fn long_folder_paths_preserve_the_project_name() {
        struct LongPathEnv;

        impl crate::flow::Env for LongPathEnv {
            fn is_dir(&self, _path: &std::path::Path) -> bool {
                true
            }

            fn scan_projects(&self) -> crate::discovery::ProjectScan {
                crate::discovery::ProjectScan {
                    paths: vec![PathBuf::from(
                        "/a/very/long/path/that/needs/truncation/project-name",
                    )],
                    visited_dirs: 1,
                    truncated: false,
                }
            }

            fn autocomplete_directories(
                &self,
                _input: &str,
            ) -> Vec<crate::discovery::DirectorySuggestion> {
                Vec::new()
            }

            fn save(&self, _spaces: &[Space]) -> Result<(), String> {
                Ok(())
            }
        }

        let mut spaces = spaces();
        let env = LongPathEnv;
        let mut picker = PickerState::new();
        for character in "new space".chars() {
            picker.on_key(Key::Char(character), &mut spaces, &env);
        }
        picker.on_key(Key::Enter, &mut spaces, &env);
        for character in "name".chars() {
            picker.on_key(Key::Char(character), &mut spaces, &env);
        }
        picker.on_key(Key::Enter, &mut spaces, &env);
        picker.on_key(Key::Enter, &mut spaces, &env);
        let lines = body_lines(&picker, &spaces, 32, 4);
        let rendered = lines[2]
            .iter()
            .map(|(text, _)| text.as_str())
            .collect::<String>();
        assert!(rendered.contains("project-name"), "{rendered}");
    }

    #[test]
    fn header_segments_right_align_the_count() {
        let segments = header_segments("\u{1f511} keyway", "3", 20);
        let rendered = segments
            .iter()
            .map(|(text, _)| text.as_str())
            .collect::<String>();
        assert_eq!(rendered.width(), 19);
        assert!(rendered.starts_with(" \u{1f511} keyway"), "{rendered}");
        assert!(rendered.ends_with("3"), "{rendered}");
    }
}
