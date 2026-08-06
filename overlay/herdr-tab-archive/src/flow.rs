//! Archived-tab picker state machine, as pure data.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub raw_idx: usize,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    Close,
    Unarchive(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PickerState {
    query: String,
    selected: usize,
}

impl PickerState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn filtered_rows<'a>(&self, rows: &'a [Row]) -> Vec<&'a Row> {
        let query = self.query.to_lowercase();
        rows.iter()
            .filter(|row| {
                query.is_empty()
                    || row.label.to_lowercase().contains(&query)
                    || row.detail.to_lowercase().contains(&query)
            })
            .collect()
    }

    pub fn on_key(&mut self, key: Key, rows: &[Row]) -> Action {
        match key {
            Key::Esc => return Action::Close,
            Key::Backspace => {
                self.query.pop();
                self.selected = 0;
            }
            Key::Char(c) => {
                self.query.push(c);
                self.selected = 0;
            }
            Key::Up => {
                let count = self.filtered_rows(rows).len();
                if count > 0 {
                    self.selected = if self.selected == 0 {
                        count - 1
                    } else {
                        self.selected - 1
                    };
                }
            }
            Key::Down => {
                let count = self.filtered_rows(rows).len();
                if count > 0 {
                    self.selected = (self.selected + 1) % count;
                }
            }
            Key::Enter => {
                let filtered = self.filtered_rows(rows);
                if let Some(row) = filtered.get(self.selected.min(filtered.len().saturating_sub(1)))
                {
                    return Action::Unarchive(row.raw_idx);
                }
            }
        }
        Action::None
    }
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
    fn typing_filters_by_label_or_public_id() {
        let rows = rows();
        let mut picker = PickerState::new();
        for c in "LOG".chars() {
            picker.on_key(Key::Char(c), &rows);
        }
        assert_eq!(picker.filtered_rows(&rows)[0].raw_idx, 1);

        let mut picker = PickerState::new();
        for c in "t5".chars() {
            picker.on_key(Key::Char(c), &rows);
        }
        assert_eq!(picker.filtered_rows(&rows)[0].raw_idx, 4);
    }

    #[test]
    fn enter_returns_the_selected_raw_index() {
        let rows = rows();
        let mut picker = PickerState::new();
        assert_eq!(picker.on_key(Key::Down, &rows), Action::None);
        assert_eq!(picker.on_key(Key::Enter, &rows), Action::Unarchive(4));
    }

    #[test]
    fn arrows_wrap_and_backspace_resets_selection() {
        let rows = rows();
        let mut picker = PickerState::new();
        picker.on_key(Key::Up, &rows);
        assert_eq!(picker.selected(), 1);
        picker.on_key(Key::Char('r'), &rows);
        assert_eq!(picker.selected(), 0);
        picker.on_key(Key::Backspace, &rows);
        assert_eq!(picker.query(), "");
    }

    #[test]
    fn escape_closes() {
        assert_eq!(PickerState::new().on_key(Key::Esc, &rows()), Action::Close);
    }
}
