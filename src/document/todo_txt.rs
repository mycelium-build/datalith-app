use std::collections::HashSet;
use std::path::{Path, PathBuf};

use txtodo::{
    Priority, SortDirection, Task, TaskFilter, TaskFilters, TaskPatch, TaskSorter, TaskSorts,
    TodoOptions, TodoTxt, TodoTxtParser, TodoTxtSerializer,
};

use super::handler::ReloadOutcome;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FilterKind {
    All,
    Incomplete,
    Completed,
    PriorityA,
    PriorityB,
    PriorityC,
}

impl FilterKind {
    pub(crate) const ALL: &[Self] = &[
        Self::All,
        Self::Incomplete,
        Self::Completed,
        Self::PriorityA,
        Self::PriorityB,
        Self::PriorityC,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Incomplete => "Incomplete",
            Self::Completed => "Completed",
            Self::PriorityA => "Priority A",
            Self::PriorityB => "Priority B",
            Self::PriorityC => "Priority C",
        }
    }

    pub(crate) fn from_index(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or(Self::All)
    }

    fn predicate(self) -> Option<TaskFilter> {
        match self {
            Self::All => None,
            Self::Incomplete => Some(TaskFilters::incomplete()),
            Self::Completed => Some(TaskFilters::completed()),
            Self::PriorityA => Some(TaskFilters::by_priority(Priority('A'))),
            Self::PriorityB => Some(TaskFilters::by_priority(Priority('B'))),
            Self::PriorityC => Some(TaskFilters::by_priority(Priority('C'))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SortKind {
    Priority,
    DateCreated,
    Description,
    Project,
    Context,
}

impl SortKind {
    pub(crate) const ALL: &[Self] = &[
        Self::Priority,
        Self::DateCreated,
        Self::Description,
        Self::Project,
        Self::Context,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Priority => "Priority",
            Self::DateCreated => "Date",
            Self::Description => "Description",
            Self::Project => "Project",
            Self::Context => "Context",
        }
    }

    pub(crate) fn from_index(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }

    fn sorter(self, descending: bool) -> TaskSorter {
        let direction = if descending {
            SortDirection::Desc
        } else {
            SortDirection::Asc
        };
        match self {
            Self::Priority => TaskSorts::by_priority(direction),
            Self::DateCreated => TaskSorts::by_date_created(direction),
            Self::Description => TaskSorts::by_description(direction),
            Self::Project => TaskSorts::by_project(direction),
            Self::Context => TaskSorts::by_context(direction),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FocusTarget {
    Task(usize),
    Search,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MutationOutcome {
    pub(crate) focus: Option<FocusTarget>,
}

pub(crate) struct TodoTxtWorkspace {
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
    pub(crate) fn open(path: &Path) -> Self {
        let path = path.to_path_buf();
        let mut todo = TodoTxt::new(TodoOptions {
            file_path: Some(path.to_string_lossy().to_string()),
            auto_save: true,
            ..Default::default()
        })
        .expect("Failed to create TodoTxt");
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

    pub(crate) fn tasks(&self) -> Vec<&Task> {
        self.todo.list()
    }
    pub(crate) fn task(&self, index: usize) -> Option<&Task> {
        self.todo.list().get(index).copied()
    }
    pub(crate) fn task_count(&self) -> usize {
        self.todo.list().len()
    }
    pub(crate) fn completed_count(&self) -> usize {
        self.todo
            .list()
            .iter()
            .filter(|task| task.completed)
            .count()
    }
    pub(crate) fn parse_error_count(&self) -> usize {
        self.parse_errors.len()
    }
    pub(crate) fn selected(&self) -> Option<usize> {
        self.selected
    }
    pub(crate) fn is_expanded(&self, index: usize) -> bool {
        self.expanded.contains(&index)
    }
    pub(crate) fn sort_descending(&self) -> bool {
        self.sort_descending
    }

    pub(crate) fn set_search_query(&mut self, query: String) {
        self.search_query = query;
    }
    pub(crate) fn set_filter(&mut self, filter: FilterKind) {
        self.filter = filter;
    }
    pub(crate) fn set_sort(&mut self, sort: Option<SortKind>) {
        self.sort = sort;
    }
    pub(crate) fn toggle_sort_direction(&mut self) {
        self.sort_descending = !self.sort_descending;
    }
    pub(crate) fn toggle_expanded(&mut self, index: usize) {
        if !self.expanded.remove(&index) {
            self.expanded.insert(index);
        }
    }

    pub(crate) fn reload_from_disk(&mut self) -> anyhow::Result<ReloadOutcome> {
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

    pub(crate) fn add_task(&mut self, description: &str) {
        let description = description.trim();
        if description.is_empty() {
            return;
        }
        if let Err(error) = self.todo.add(format!("{} {description}", today_string())) {
            self.parse_errors.push(format!("Add task failed: {error}"));
        }
    }

    pub(crate) fn toggle_complete(&mut self, index: usize) {
        if let Some(task) = self.task(index) {
            if task.completed {
                let _ = self.todo.unmark([index as i64]);
            } else {
                let _ = self.todo.mark([index as i64]);
            }
        }
    }

    pub(crate) fn add_subtask(&mut self, parent_index: usize) -> MutationOutcome {
        let Some(parent) = self.task(parent_index) else {
            return MutationOutcome::default();
        };
        let insert_at = parent_index + 1;
        let raw = format!(
            "{}{} New subtask",
            " ".repeat(parent.indent_level + 1),
            today_string()
        );
        if let Err(error) = self.todo.insert(insert_at as i64, raw) {
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

    pub(crate) fn delete_task(&mut self, index: usize) -> MutationOutcome {
        let old_len = self.task_count();
        let _ = self.todo.remove([index as i64]);
        let new_len = self.task_count();
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

    pub(crate) fn update_description(&mut self, index: usize, value: &str) {
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

    pub(crate) fn update_date(&mut self, index: usize, value: &str) {
        self.update(
            index,
            TaskPatch {
                creation_date: Some(parse_date(value)),
                ..Default::default()
            },
        );
    }

    pub(crate) fn update_priority(&mut self, index: usize, value: &str) {
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
        if let Err(error) = self.todo.update(index as i64, patch) {
            self.parse_errors.push(format!("Save failed: {error}"));
        }
    }

    pub(crate) fn visible_tasks(&self) -> Vec<(usize, bool)> {
        let flat = self.todo.list();
        let query = self.search_query.to_lowercase();
        let filter = self.filter.predicate();
        let mut subtree_match = vec![false; flat.len()];
        for index in (0..flat.len()).rev() {
            let task = &flat[index];
            let own_match = filter.as_ref().is_none_or(|predicate| predicate(task))
                && (query.is_empty() || matches_task(task, &query));
            let child_match = ((index + 1)..flat.len())
                .take_while(|&child| flat[child].indent_level > task.indent_level)
                .any(|child| subtree_match[child]);
            subtree_match[index] = own_match || child_match;
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
            if !subtree_match[index] {
                continue;
            }
            visible.push(index);
            if !task.subtasks.is_empty() && !self.expanded.contains(&index) {
                collapsed_depth = Some(task.indent_level);
            }
        }
        if let Some(sort) = self.sort {
            let sorter = sort.sorter(self.sort_descending);
            let top_level: Vec<_> = visible
                .iter()
                .copied()
                .filter(|&i| flat[i].indent_level == 0)
                .collect();
            let mut groups = Vec::new();
            for top in top_level {
                let position = visible
                    .iter()
                    .position(|&i| i == top)
                    .expect("top-level task is visible");
                let mut group = vec![top];
                group.extend(
                    visible[position + 1..]
                        .iter()
                        .copied()
                        .take_while(|&i| flat[i].indent_level > flat[top].indent_level),
                );
                groups.push((top, group));
            }
            groups.sort_by(|(a, _), (b, _)| sorter(flat[*a], flat[*b]));
            visible = groups.into_iter().flat_map(|(_, group)| group).collect();
        }
        visible
            .into_iter()
            .map(|index| {
                (
                    index,
                    !flat[index].subtasks.is_empty() && self.expanded.contains(&index),
                )
            })
            .collect()
    }

    fn shift_expanded_after_insert(&mut self, at: usize) {
        self.expanded = std::mem::take(&mut self.expanded)
            .into_iter()
            .map(|index| if index >= at { index + 1 } else { index })
            .collect();
    }
}

pub(crate) fn parse_date(value: &str) -> Option<time::Date> {
    let mut parts = value.split('-');
    let year = parts.next()?.parse().ok()?;
    let month = parts.next()?.parse::<u8>().ok()?;
    let day = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    time::Date::from_calendar_date(year, time::Month::try_from(month).ok()?, day).ok()
}

fn today_string() -> String {
    time::OffsetDateTime::now_utc().date().to_string()
}

fn matches_task(task: &Task, query: &str) -> bool {
    task.description.to_lowercase().contains(query)
        || task
            .priority
            .as_ref()
            .is_some_and(|p| p.to_string().to_lowercase().contains(query))
        || task
            .creation_date
            .is_some_and(|d| d.to_string().to_lowercase().contains(query))
        || task
            .projects
            .iter()
            .any(|p| p.to_lowercase().contains(query))
        || task
            .contexts
            .iter()
            .any(|c| c.to_lowercase().contains(query))
        || task.extensions.iter().any(|(k, v)| {
            k.to_lowercase().contains(query) || v.to_string().to_lowercase().contains(query)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

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
