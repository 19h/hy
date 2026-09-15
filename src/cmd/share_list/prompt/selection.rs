//! Questionary checkbox state: substring filtering and selection across filters.

pub(super) enum Edit {
    Type(char),
    Erase,
    Next,
    Previous,
    Toggle,
    ToggleAll,
    Invert,
}

pub(super) struct Selection {
    names: Vec<String>,
    // Inversion preserves equal-value occurrences; toggling removes one.
    selected_values: Vec<usize>,
    groups: Vec<usize>,
    pub(super) visible: Vec<usize>,
    pub(super) query: String,
    pub(super) cursor: usize,
    pub(super) found_matches: bool,
}

impl Selection {
    pub(super) fn new<T: PartialEq>(choices: &[String], values: &[T]) -> Self {
        assert_eq!(choices.len(), values.len());
        let groups = values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                values[..index].iter().position(|previous| previous == value).unwrap_or(index)
            })
            .collect();
        Self {
            names: choices.iter().map(|name| name.to_lowercase()).collect(),
            selected_values: Vec::new(),
            groups,
            visible: (0..choices.len()).collect(),
            query: String::new(),
            cursor: 0,
            found_matches: true,
        }
    }

    pub(super) fn apply(&mut self, edit: Edit) {
        match edit {
            Edit::Type(character) => {
                self.query.push(character);
                self.filter();
            }
            Edit::Erase => {
                self.query.pop();
                self.filter();
            }
            Edit::Next if !self.visible.is_empty() => {
                self.cursor = (self.cursor + 1) % self.visible.len()
            }
            Edit::Previous if !self.visible.is_empty() => {
                self.cursor = (self.cursor + self.visible.len() - 1) % self.visible.len()
            }
            Edit::Toggle => {
                if let Some(&index) = self.visible.get(self.cursor) {
                    let group = self.groups[index];
                    if let Some(position) =
                        self.selected_values.iter().position(|value| *value == group)
                    {
                        self.selected_values.remove(position);
                    } else {
                        self.selected_values.push(group);
                    }
                }
            }
            Edit::ToggleAll => {
                let mut all_selected = true;
                for group in &self.groups {
                    if !self.selected_values.contains(group) {
                        self.selected_values.push(*group);
                        all_selected = false;
                    }
                }
                if all_selected {
                    self.selected_values.clear();
                }
            }
            Edit::Invert => {
                self.selected_values = self
                    .groups
                    .iter()
                    .filter(|group| !self.selected_values.contains(group))
                    .copied()
                    .collect();
            }
            _ => {}
        }
    }

    fn filter(&mut self) {
        let query = self.query.to_lowercase();
        self.visible = self
            .names
            .iter()
            .enumerate()
            .filter_map(|(index, name)| name.contains(&query).then_some(index))
            .collect();
        self.found_matches = !self.visible.is_empty();
        if !self.found_matches {
            self.visible = (0..self.names.len()).collect();
        }
        self.cursor = 0;
    }

    pub(super) fn selected(&self) -> Vec<usize> {
        self.groups
            .iter()
            .enumerate()
            .filter_map(|(index, group)| self.selected_values.contains(group).then_some(index))
            .collect()
    }

    pub(super) fn is_selected(&self, index: usize) -> bool {
        self.selected_values.contains(&self.groups[index])
    }
}

#[cfg(test)]
mod tests;
