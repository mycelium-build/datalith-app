use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use txtodo::{Priority, Task, TaskPatch, TodoOptions, TodoTxt, TodoTxtParser, TodoTxtSerializer};

use super::{FilterKind, FocusTarget, MutationOutcome, SortKind, matches_task, today_string};
use crate::document::handler::ReloadOutcome;

pub struct TodoTxtWorkspace {
    todo: TodoTxt,
    path: PathBuf,
    search_query: String,
    filter: FilterKind,
    sort: Option<SortKind>,
    sort_descending: bool,
    expanded: HashSet<usize>,
    selected: Option<usize>,
    parse_errors: Vec<String>,
}

impl TodoTxtWorkspace {
    pub fn open(path: &Path) -> Self {
        let path = path.to_path_buf();
        let mut todo = {
            let options = TodoOptions {
                file_path: Some(path.to_string_lossy().to_string()),
                auto_save: true,
                ..Default::default()
            };
            // `TodoTxt::new` only fails when a configured extension fails to initialize; the default options register no extensions.
            #[allow(clippy::expect_used)]
            TodoTxt::new(options).expect("Failed to create TodoTxt")
        };
        let parse_errors = todo
            .load(None)
            .err()
            .map(|e| e.to_string())
            .into_iter()
            .collect();
        let expanded = todo
            .list()
            .iter()
            .enumerate()
            .filter_map(|(index, task)| (!task.subtasks.is_empty()).then_some(index))
            .collect();
        Self {
            todo,
            path,
            search_query: String::new(),
            filter: FilterKind::All,
            sort: Some(SortKind::DateCreated),
            sort_descending: false,
            expanded,
            selected: None,
            parse_errors,
        }
    }

    pub fn tasks(&self) -> Vec<&Task> {
        self.todo.list()
    }
    pub fn task(&self, index: usize) -> Option<&Task> {
        self.todo.list().get(index).copied()
    }
    pub fn task_count(&self) -> usize {
        self.todo.list().len()
    }
    pub fn completed_count(&self) -> usize {
        self.todo
            .list()
            .iter()
            .filter(|task| task.completed)
            .count()
    }
    pub const fn parse_error_count(&self) -> usize {
        self.parse_errors.len()
    }
    pub const fn selected(&self) -> Option<usize> {
        self.selected
    }
    pub fn is_expanded(&self, index: usize) -> bool {
        self.expanded.contains(&index)
    }
    pub const fn sort_descending(&self) -> bool {
        self.sort_descending
    }

    pub fn set_search_query(&mut self, query: String) {
        self.search_query = query;
    }
    pub const fn set_filter(&mut self, filter: FilterKind) {
        self.filter = filter;
    }
    pub const fn set_sort(&mut self, sort: Option<SortKind>) {
        self.sort = sort;
    }
    pub const fn toggle_sort_direction(&mut self) {
        self.sort_descending = !self.sort_descending;
    }
    pub fn toggle_expanded(&mut self, index: usize) {
        if !self.expanded.remove(&index) {
            self.expanded.insert(index);
        }
    }

    pub fn reload_from_disk(&mut self) -> anyhow::Result<ReloadOutcome> {
        let disk_content = std::fs::read_to_string(&self.path)?;
        let current_content = TodoTxtSerializer::new()
            .serialize_tasks(&self.todo.tasks)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        if disk_content == current_content {
            return Ok(ReloadOutcome::Unchanged);
        }
        if let Err(error) = self.todo.load(None) {
            self.parse_errors = vec![error.to_string()];
            return Err(anyhow::anyhow!(error.to_string()));
        }
        self.parse_errors.clear();
        self.expanded = self
            .todo
            .list()
            .iter()
            .enumerate()
            .filter_map(|(index, task)| (!task.subtasks.is_empty()).then_some(index))
            .collect();
        if self
            .selected
            .is_some_and(|index| index >= self.task_count())
        {
            self.selected = self.task_count().checked_sub(1);
        }
        Ok(ReloadOutcome::Reloaded)
    }

    pub fn add_task(&mut self, description: &str) {
        let description = description.trim();
        if description.is_empty() {
            return;
        }
        if let Err(error) = self.todo.add(format!("{} {description}", today_string())) {
            self.parse_errors.push(format!("Add task failed: {error}"));
        }
    }

    pub fn toggle_complete(&mut self, index: usize) {
        if let Some(task) = self.task(index) {
            if task.completed {
                let _ = self.todo.unmark([index_as_i64(index)]);
            } else {
                let _ = self.todo.mark([index_as_i64(index)]);
            }
        }
    }

    pub fn add_subtask(&mut self, parent_index: usize) -> MutationOutcome {
        let Some(parent) = self.task(parent_index) else {
            return MutationOutcome::default();
        };
        // Indices and indent levels are bounded by the task list size,
        // so the additions below cannot overflow.
        #[allow(clippy::arithmetic_side_effects)]
        {
            let insert_at = parent_index + 1;
            let raw = format!(
                "{}{} New subtask",
                " ".repeat(parent.indent_level + 1),
                today_string()
            );
            if let Err(error) = self.todo.insert(index_as_i64(insert_at), raw) {
                self.parse_errors
                    .push(format!("Add subtask failed: {error}"));
                return MutationOutcome::default();
            }
            self.shift_expanded_after_insert(insert_at);
            self.expanded.insert(parent_index);
            if self.selected.is_some_and(|selected| selected >= insert_at) {
                self.selected = self.selected.map(|selected| selected + 1);
            }
            MutationOutcome {
                focus: Some(FocusTarget::Task(insert_at)),
            }
        }
    }

    pub fn delete_task(&mut self, index: usize) -> MutationOutcome {
        let old_len = self.task_count();
        let _ = self.todo.remove([index_as_i64(index)]);
        let new_len = self.task_count();
        // Index arithmetic below is bounded by the task list size,
        // and every subtraction is guarded by a preceding range check.
        #[allow(clippy::arithmetic_side_effects)]
        {
            let removed = old_len.saturating_sub(new_len);
            let old_expanded = std::mem::take(&mut self.expanded);
            for expanded in old_expanded {
                if expanded < index {
                    self.expanded.insert(expanded);
                } else if expanded >= index + removed {
                    self.expanded.insert(expanded - removed);
                }
            }
            if let Some(selected) = self.selected {
                if selected >= new_len && new_len > 0 {
                    self.selected = Some(new_len - 1);
                } else if selected >= index + removed {
                    self.selected = Some(selected - removed);
                }
            }
            let focus = if new_len == 0 {
                Some(FocusTarget::Search)
            } else if index > 0 {
                Some(FocusTarget::Task(index - 1))
            } else {
                Some(FocusTarget::Task(0))
            };
            MutationOutcome { focus }
        }
    }

    pub fn update_description(&mut self, index: usize, value: &str) {
        let parsed = TodoTxtParser::new().parse_line(value).ok();
        self.update(
            index,
            TaskPatch {
                description: Some(
                    parsed
                        .as_ref()
                        .map_or_else(|| value.to_string(), |t| t.description.clone()),
                ),
                projects: parsed.as_ref().map(|t| t.projects.clone()),
                contexts: parsed.as_ref().map(|t| t.contexts.clone()),
                extensions: parsed.map(|t| t.extensions),
                ..Default::default()
            },
        );
    }

    pub fn update_date(&mut self, index: usize, value: &str) {
        self.update(
            index,
            TaskPatch {
                creation_date: Some(super::parse_date(value)),
                ..Default::default()
            },
        );
    }

    pub fn update_priority(&mut self, index: usize, value: &str) {
        let priority = if value.is_empty() {
            None
        } else {
            Priority::from_token(value).ok()
        };
        self.update(
            index,
            TaskPatch {
                priority: Some(priority),
                ..Default::default()
            },
        );
    }

    fn update(&mut self, index: usize, patch: TaskPatch) {
        if let Err(error) = self.todo.update(index_as_i64(index), patch) {
            self.parse_errors.push(format!("Save failed: {error}"));
        }
    }

    pub fn visible_tasks(&self) -> Vec<(usize, bool)> {
        let flat = self.todo.list();
        let query = self.search_query.to_lowercase();
        let filter = self.filter.predicate();
        let mut subtree_match = vec![false; flat.len()];
        // Indices are bounded by `flat.len()`,
        // so the arithmetic below cannot overflow.
        #[allow(clippy::arithmetic_side_effects)]
        for index in (0..flat.len()).rev() {
            let Some(task) = flat.get(index) else {
                continue;
            };
            let own_match = filter.as_ref().is_none_or(|predicate| predicate(task))
                && (query.is_empty() || matches_task(task, &query));
            let child_match = ((index + 1)..flat.len())
                .take_while(|&child| {
                    flat.get(child)
                        .is_some_and(|child_task| child_task.indent_level > task.indent_level)
                })
                .any(|child| subtree_match.get(child).copied().unwrap_or(false));
            if let Some(cell) = subtree_match.get_mut(index) {
                *cell = own_match || child_match;
            }
        }
        let mut visible = Vec::new();
        let mut collapsed_depth = None;
        for (index, task) in flat.iter().enumerate() {
            if let Some(depth) = collapsed_depth {
                if task.indent_level > depth {
                    continue;
                }
                collapsed_depth = None;
            }
            if !subtree_match.get(index).copied().unwrap_or(false) {
                continue;
            }
            visible.push(index);
            if !task.subtasks.is_empty() && !self.expanded.contains(&index) {
                collapsed_depth = Some(task.indent_level);
            }
        }
        if let Some(sort) = self.sort {
            let sorter = sort.sorter(self.sort_descending);
            let mut groups: Vec<(usize, Vec<usize>)> = Vec::new();
            for &index in &visible {
                let Some(task) = flat.get(index) else {
                    continue;
                };
                if task.indent_level == 0 {
                    groups.push((index, vec![index]));
                } else if let Some((_, group)) = groups.last_mut() {
                    group.push(index);
                }
            }
            groups.sort_by(|(a, _), (b, _)| {
                let Some(left) = flat.get(*a) else {
                    return Ordering::Equal;
                };
                let Some(right) = flat.get(*b) else {
                    return Ordering::Equal;
                };
                sorter(left, right)
            });
            visible = groups.into_iter().flat_map(|(_, group)| group).collect();
        }
        visible
            .into_iter()
            .map(|index| {
                (
                    index,
                    flat.get(index).is_some_and(|task| {
                        !task.subtasks.is_empty() && self.expanded.contains(&index)
                    }),
                )
            })
            .collect()
    }

    fn shift_expanded_after_insert(&mut self, at: usize) {
        // `at` is a task index bounded by the task list size.
        #[allow(clippy::arithmetic_side_effects)]
        {
            self.expanded = std::mem::take(&mut self.expanded)
                .into_iter()
                .map(|index| if index >= at { index + 1 } else { index })
                .collect();
        }
    }
}

const fn index_as_i64(index: usize) -> i64 {
    // Task indices are positions within an in-memory task list, which never exceed the range of an i64 in practice.
    #[allow(clippy::cast_possible_wrap, clippy::as_conversions)]
    {
        index as i64
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::indexing_slicing,
        clippy::string_slice
    )]

    use super::*;
    use crate::document::todo_txt::FocusTarget;

    fn workspace(contents: &str) -> (TodoTxtWorkspace, PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "datalith-todo-{}-{}.txt",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, contents).unwrap();
        (TodoTxtWorkspace::open(&path), path)
    }

    #[test]
    fn matching_child_keeps_parent_visible() {
        let (mut workspace, path) = workspace("Parent\n    matching child\nOther\n");
        assert_eq!(
            workspace
                .tasks()
                .iter()
                .map(|task| (task.description.as_str(), task.indent_level))
                .collect::<Vec<_>>(),
            vec![("Parent", 0), ("matching child", 4), ("Other", 0)]
        );
        workspace.set_search_query("matching".into());
        assert_eq!(
            workspace
                .visible_tasks()
                .iter()
                .map(|v| v.0)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn deleting_parent_repairs_expansion_and_focus() {
        let (mut workspace, path) = workspace("Parent\n    Child\nNext\n");
        let outcome = workspace.delete_task(0);
        assert_eq!(workspace.task_count(), 1);
        assert_eq!(outcome.focus, Some(FocusTarget::Task(0)));
        assert!(!workspace.is_expanded(0));
        let _ = std::fs::remove_file(path);
    }
}
