//! Presentation data for the archived-tab picker.

use unicode_width::UnicodeWidthStr;

use crate::{PickerState, Row};

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
}

pub type Segment = (String, Tone);

pub fn body_lines(
    picker: &PickerState,
    rows: &[Row],
    width: u16,
    max_rows: usize,
) -> Vec<Vec<Segment>> {
    let width = usize::from(width);
    let mut lines = vec![input_line(picker), separator(width)];
    let filtered = picker.filtered_rows(rows);
    if filtered.is_empty() {
        lines.push(vec![(" no matches".into(), Tone::Dim)]);
        return lines;
    }

    let selected = picker.selected().min(filtered.len().saturating_sub(1));
    let start = selected.saturating_sub(max_rows.saturating_sub(1));
    for (idx, row) in filtered.iter().enumerate().skip(start).take(max_rows) {
        let highlighted = idx == selected;
        let (label_tone, detail_tone) = if highlighted {
            (Tone::RowSelected, Tone::DetailSelected)
        } else {
            (Tone::Row, Tone::Detail)
        };
        let marker = if highlighted { ">" } else { " " };
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
    let mut spans = vec![(" / ".to_string(), Tone::Accent)];
    if picker.query().is_empty() {
        spans.push(("filter".to_string(), Tone::Dim));
    } else {
        spans.push((picker.query().to_string(), Tone::Text));
    }
    spans
}

fn separator(width: usize) -> Vec<Segment> {
    vec![("-".repeat(width), Tone::Separator)]
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

    fn rows() -> Vec<Row> {
        vec![
            Row {
                raw_idx: 1,
                label: "build logs".into(),
                detail: "w1:t2".into(),
            },
            Row {
                raw_idx: 4,
                label: "review".into(),
                detail: "w1:t5".into(),
            },
        ]
    }

    #[test]
    fn rows_are_padded_to_modal_width() {
        let lines = body_lines(&PickerState::new(), &rows(), 30, 8);
        let rendered = lines[2]
            .iter()
            .map(|(text, _)| text.as_str())
            .collect::<String>();
        assert_eq!(rendered.width(), 30);
        assert!(rendered.starts_with("> build logs"), "{rendered}");
        assert!(rendered.ends_with("w1:t2"), "{rendered}");
    }

    #[test]
    fn only_selected_row_is_highlighted() {
        let lines = body_lines(&PickerState::new(), &rows(), 30, 8);
        assert_eq!(lines[2][0].1, Tone::RowSelected);
        assert_eq!(lines[3][0].1, Tone::Row);
    }

    #[test]
    fn empty_filter_result_is_explicit() {
        let rows = rows();
        let mut picker = PickerState::new();
        for c in "missing".chars() {
            picker.on_key(crate::Key::Char(c), &rows);
        }
        assert_eq!(body_lines(&picker, &rows, 30, 8)[2][0].0, " no matches");
    }
}
