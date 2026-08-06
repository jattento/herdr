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
    // The error line eats one row of the body.
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
            lines.push(vec![(" no matches".to_string(), Tone::Dim)]);
        }
        return lines;
    }

    let selected = picker.selected().min(rows.len().saturating_sub(1));
    let start = selected.saturating_sub(max_rows.saturating_sub(1));
    for (idx, row) in rows.iter().enumerate().skip(start).take(max_rows) {
        let highlighted = idx == selected;
        let (label_tone, detail_tone) = if highlighted {
            (Tone::RowSelected, Tone::DetailSelected)
        } else {
            (Tone::Row, Tone::Detail)
        };
        let marker = if highlighted { "\u{203a}" } else { " " };
        let label = truncate(&format!("{marker} {}", row.label), width);
        let detail = truncate(&row.detail, width.saturating_sub(label.width()));
        let pad = width
            .saturating_sub(label.width())
            .saturating_sub(detail.width());
        lines.push(vec![
            (label, label_tone),
            (" ".repeat(pad), label_tone),
            (detail, detail_tone),
        ]);
    }
    lines
}

fn input_line(picker: &PickerState) -> Vec<Segment> {
    if picker.step().is_text_input() {
        return vec![
            (format!(" {} ", picker.step().prompt()), Tone::Dim),
            (format!("{}\u{2588}", picker.input()), Tone::Text),
        ];
    }

    let mut spans = vec![(" / ".to_string(), Tone::Accent)];
    if picker.query().is_empty() {
        spans.push(("filter".to_string(), Tone::Dim));
    } else {
        spans.push((picker.query().to_string(), Tone::Text));
    }
    if let Some(folder) = picker.folder() {
        spans.push((format!("  {}", folder.display()), Tone::Dim));
    }
    spans
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
        assert_eq!(lines.len(), 5);
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
        assert_eq!(lines.len(), 3);
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
        assert_eq!(lines.len(), 2);
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
