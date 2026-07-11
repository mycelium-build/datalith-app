use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::checkbox::Checkbox;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::popover::{Popover, PopoverState};
use gpui_component::select::{Select, SelectEvent, SelectState};
use gpui_component::{
    ActiveTheme, Icon, IconName, IndexPath, Selectable, Sizable, VirtualListScrollHandle,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex, v_virtual_list,
};
use txtodo::{
    Priority, SortDirection, Task, TaskFilter, TaskFilters, TaskPatch, TaskSorter, TaskSorts,
    TodoOptions, TodoTxt, TodoTxtParser,
};

use crate::assets::{ARROW_DOWN_AZ_ICON, ARROW_UP_AZ_ICON, FUNNEL_ICON};

const TODO_ROW_HEIGHT: f32 = 32.0;
const TODO_HEADER_HEIGHT: f32 = 40.0;
const TODO_PILL_PADDING_H: f32 = 6.0;
const TODO_PILL_RADIUS: f32 = 4.0;
const TODO_INDENT_PX: f32 = 20.0;
const TODO_NEW_ROW_HEIGHT: f32 = 36.0;
const TODO_COL_EXPAND: f32 = 24.0;
const TODO_COL_CHECK: f32 = 24.0;
const TODO_COL_PRIORITY: f32 = 56.0;
const TODO_COL_DATE: f32 = 90.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FilterKind {
    All,
    Incomplete,
    Completed,
    PriorityA,
    PriorityB,
    PriorityC,
}

impl FilterKind {
    const ALL: &[FilterKind] = &[
        FilterKind::All,
        FilterKind::Incomplete,
        FilterKind::Completed,
        FilterKind::PriorityA,
        FilterKind::PriorityB,
        FilterKind::PriorityC,
    ];

    fn label(self) -> &'static str {
        match self {
            FilterKind::All => "All",
            FilterKind::Incomplete => "Incomplete",
            FilterKind::Completed => "Completed",
            FilterKind::PriorityA => "Priority A",
            FilterKind::PriorityB => "Priority B",
            FilterKind::PriorityC => "Priority C",
        }
    }

    fn from_index(ix: usize) -> Self {
        Self::ALL.get(ix).copied().unwrap_or(FilterKind::All)
    }

    #[allow(dead_code)]
    fn to_filter(self) -> Option<TaskFilter> {
        match self {
            FilterKind::All => None,
            FilterKind::Incomplete => Some(TaskFilters::incomplete()),
            FilterKind::Completed => Some(TaskFilters::completed()),
            FilterKind::PriorityA => Some(TaskFilters::by_priority(Priority('A'))),
            FilterKind::PriorityB => Some(TaskFilters::by_priority(Priority('B'))),
            FilterKind::PriorityC => Some(TaskFilters::by_priority(Priority('C'))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortKind {
    Priority,
    DateCreated,
    Description,
    Project,
    Context,
}

impl SortKind {
    const ALL: &[SortKind] = &[
        SortKind::Priority,
        SortKind::DateCreated,
        SortKind::Description,
        SortKind::Project,
        SortKind::Context,
    ];

    fn label(self) -> &'static str {
        match self {
            SortKind::Priority => "Priority",
            SortKind::DateCreated => "Date",
            SortKind::Description => "Description",
            SortKind::Project => "Project",
            SortKind::Context => "Context",
        }
    }

    fn from_index(ix: usize) -> Option<Self> {
        Self::ALL.get(ix).copied()
    }

    fn to_sorter(self, desc: bool) -> TaskSorter {
        let dir = if desc {
            SortDirection::Desc
        } else {
            SortDirection::Asc
        };
        match self {
            SortKind::Priority => TaskSorts::by_priority(dir),
            SortKind::DateCreated => TaskSorts::by_date_created(dir),
            SortKind::Description => TaskSorts::by_description(dir),
            SortKind::Project => TaskSorts::by_project(dir),
            SortKind::Context => TaskSorts::by_context(dir),
        }
    }
}

#[derive(IntoElement)]
struct PriorityTrigger {
    id: ElementId,
    editor: Entity<TodoTxtState>,
    task_index: usize,
    current: Option<Priority>,
    completed: bool,
}

impl Selectable for PriorityTrigger {
    fn selected(self, _selected: bool) -> Self {
        self
    }

    fn is_selected(&self) -> bool {
        false
    }
}

impl RenderOnce for PriorityTrigger {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let color = self.current.map(|p| {
            if self.completed {
                cx.theme().muted_foreground
            } else {
                priority_color(p.as_char())
            }
        });
        let label = self
            .current
            .map(|p| format!("({})", p.as_char()))
            .unwrap_or_default();

        let task_index = self.task_index;
        let editor = self.editor;
        let pill = div()
            .id(self.id)
            .flex_shrink_0()
            .w(px(TODO_COL_PRIORITY))
            .h(px(TODO_COL_CHECK))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(TODO_PILL_RADIUS))
            .text_sm()
            .cursor_pointer()
            .tab_index(0)
            .on_key_down(move |event: &KeyDownEvent, _window, app| {
                let key = event.keystroke.key.as_str();
                if key != "up" && key != "down" {
                    return;
                }
                let _ = app.update_entity(&editor, |this, cx| {
                    let current = this.todo.list().get(task_index).and_then(|t| t.priority);
                    let priorities: [Option<char>; 6] =
                        [None, Some('A'), Some('B'), Some('C'), Some('D'), Some('E')];
                    let current_idx = current
                        .and_then(|p| {
                            priorities
                                .iter()
                                .position(|&item| item == Some(p.as_char()))
                        })
                        .unwrap_or(0);
                    let new_idx = if key == "up" {
                        current_idx.checked_sub(1).unwrap_or(priorities.len() - 1)
                    } else {
                        (current_idx + 1) % priorities.len()
                    };
                    let value = priorities[new_idx]
                        .map(|priority| format!("({priority})"))
                        .unwrap_or_default();
                    this.commit_priority(task_index, &value, cx);
                });
            });

        if let Some(color) = color {
            pill.bg(color.opacity(0.15))
                .hover(|style| style.bg(color.opacity(0.25)))
                .focus_visible(|style| {
                    style
                        .border(px(1.5))
                        .border_color(cx.theme().ring.alpha(0.2))
                })
                .text_color(color)
                .font_weight(FontWeight::BOLD)
                .child(label)
        } else {
            pill.border_1()
                .border_color(cx.theme().muted_foreground.opacity(0.2))
                .opacity(0.0)
                .group("todo-row")
                .when(true, |el| {
                    el.group_hover("todo-row", |style| style.opacity(0.6))
                })
                .focus_visible(|style| {
                    style
                        .opacity(1.0)
                        .border(px(1.5))
                        .border_color(cx.theme().ring.alpha(0.2))
                })
                .child(
                    Icon::new(IconName::Plus)
                        .size_3()
                        .text_color(cx.theme().muted_foreground),
                )
        }
    }
}

// --- TodoTxtEditor (wrapper)

pub(crate) struct TodoTxtEditor {
    state: Entity<TodoTxtState>,
}

impl TodoTxtEditor {
    pub(crate) fn new(state: Entity<TodoTxtState>) -> Self {
        Self { state }
    }

    pub(crate) fn new_state(
        path: &Path,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<TodoTxtState> {
        let path = path.to_path_buf();
        let mut todo = TodoTxt::new(TodoOptions {
            file_path: Some(path.to_string_lossy().to_string()),
            auto_save: true,
            ..Default::default()
        })
        .expect("Failed to create TodoTxt");

        let parse_errors = match todo.load(None) {
            Ok(_) => Vec::new(),
            Err(e) => vec![e.to_string()],
        };

        let flat = todo.list();
        let mut expanded = HashSet::new();
        for (i, task) in flat.iter().enumerate() {
            if !task.subtasks.is_empty() {
                expanded.insert(i);
            }
        }
        let total = flat.len();
        drop(flat);

        cx.new(|cx| {
            let search_input =
                cx.new(|cx| InputState::new(window, cx).placeholder("Search tasks..."));
            let filter_select = cx.new(|cx| {
                SelectState::new(
                    FilterKind::ALL
                        .iter()
                        .map(|f| f.label().to_string())
                        .collect::<Vec<_>>(),
                    Some(IndexPath::new(0)),
                    window,
                    cx,
                )
            });
            let sort_select = cx.new(|cx| {
                SelectState::new(
                    SortKind::ALL
                        .iter()
                        .map(|s| s.label().to_string())
                        .collect::<Vec<_>>(),
                    Some(IndexPath::new(1)),
                    window,
                    cx,
                )
            });
            let new_task_input =
                cx.new(|cx| InputState::new(window, cx).placeholder("Add a task..."));

            let item_sizes = Rc::new(vec![
                Size {
                    width: px(0.0),
                    height: px(TODO_ROW_HEIGHT)
                };
                total
            ]);

            let mut subscriptions = Vec::new();

            subscriptions.push(cx.subscribe_in(
                &search_input,
                window,
                |this: &mut TodoTxtState, _input, event, _window, cx| {
                    if let InputEvent::Change = event {
                        this.search_query = this.search_input.read(cx).value().to_string();
                        this.refresh_item_sizes();
                        cx.notify();
                    }
                },
            ));

            subscriptions.push(cx.subscribe_in(
                &filter_select,
                window,
                |this: &mut TodoTxtState, state, _event: &SelectEvent<Vec<String>>, _window, cx| {
                    if let Some(index_path) = state.read(cx).selected_index(cx) {
                        this.filter = FilterKind::from_index(index_path.row);
                        this.refresh_item_sizes();
                        cx.notify();
                    }
                },
            ));

            subscriptions.push(cx.subscribe_in(
                &sort_select,
                window,
                |this: &mut TodoTxtState, state, _event: &SelectEvent<Vec<String>>, _window, cx| {
                    if let Some(index_path) = state.read(cx).selected_index(cx) {
                        this.sort = SortKind::from_index(index_path.row);
                        this.refresh_item_sizes();
                        cx.notify();
                    }
                },
            ));

            subscriptions.push(cx.subscribe_in(
                &new_task_input,
                window,
                |this: &mut TodoTxtState, _input, event, window, cx| {
                    if let InputEvent::PressEnter { .. } = event {
                        let value = this.new_task_input.read(cx).value();
                        let trimmed = value.trim().to_string();
                        if !trimmed.is_empty() {
                            let today = today_string();
                            let line = format!("{today} {trimmed}");
                            if let Err(e) = this.todo.add(line) {
                                this.parse_errors.push(format!("Add task failed: {e}"));
                            }
                            let _ = this.todo.save(None);
                            this.clear_row_inputs();
                            this.refresh_item_sizes();
                            cx.notify();
                        }
                        this.new_task_input.update(cx, |state, cx| {
                            state.set_value("", window, cx);
                        });
                    }
                },
            ));

            let entity = cx.entity();
            subscriptions.push(cx.intercept_keystrokes(move |event, window, cx| {
                if event.keystroke.key.as_str() == "f" && event.keystroke.modifiers.secondary() {
                    entity.update(cx, |this, cx| {
                        this.search_input.focus_handle(cx).focus(window, cx);
                    });
                    cx.stop_propagation();
                }
            }));

            TodoTxtState {
                todo,
                _path: path,
                search_query: String::new(),
                filter: FilterKind::All,
                sort: Some(SortKind::DateCreated),
                sort_desc: false,
                expanded,
                selected: None,
                priority_picker_open: None,
                pending_focus_desc: None,
                pending_focus_search: false,
                parse_errors,
                search_input,
                filter_select,
                sort_select,
                new_task_input,
                desc_inputs: HashMap::new(),
                date_inputs: HashMap::new(),
                desc_subs: HashMap::new(),
                date_subs: HashMap::new(),
                scroll_handle: VirtualListScrollHandle::new(),
                item_sizes,
                _subscriptions: subscriptions,
            }
        })
    }

    pub(crate) fn render(&self, _cx: &mut App) -> AnyElement {
        div()
            .size_full()
            .child(self.state.clone())
            .into_any_element()
    }

    pub(crate) fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.state.read(cx).search_input.focus_handle(cx)
    }
}

// --- TodoTxtState

pub(crate) struct TodoTxtState {
    todo: TodoTxt,
    _path: std::path::PathBuf,
    search_query: String,
    filter: FilterKind,
    sort: Option<SortKind>,
    sort_desc: bool,
    expanded: HashSet<usize>,
    selected: Option<usize>,
    priority_picker_open: Option<usize>,
    pending_focus_desc: Option<usize>,
    pending_focus_search: bool,
    parse_errors: Vec<String>,
    search_input: Entity<InputState>,
    filter_select: Entity<SelectState<Vec<String>>>,
    sort_select: Entity<SelectState<Vec<String>>>,
    new_task_input: Entity<InputState>,
    desc_inputs: HashMap<usize, Entity<InputState>>,
    date_inputs: HashMap<usize, Entity<InputState>>,
    desc_subs: HashMap<usize, Subscription>,
    date_subs: HashMap<usize, Subscription>,
    scroll_handle: VirtualListScrollHandle,
    item_sizes: Rc<Vec<Size<Pixels>>>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<()> for TodoTxtState {}

impl Focusable for TodoTxtState {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search_input.focus_handle(cx)
    }
}

impl TodoTxtState {
    fn visible_tasks(&self) -> Vec<(usize, bool)> {
        let flat = self.todo.list();
        let query = self.search_query.to_lowercase();
        let filter_fn = self.filter.to_filter();

        let n = flat.len();
        let mut subtree_match = vec![false; n];
        for i in (0..n).rev() {
            let task = &flat[i];
            let self_match = filter_fn.as_ref().map_or(true, |f| f(task))
                && (query.is_empty() || matches_task(task, &query));
            let child_match = {
                let mut c = false;
                for j in (i + 1)..n {
                    if flat[j].indent_level <= task.indent_level {
                        break;
                    }
                    if flat[j].indent_level == task.indent_level + 1 && subtree_match[j] {
                        c = true;
                        break;
                    }
                }
                c
            };
            subtree_match[i] = self_match || child_match;
        }

        let mut visible_indices: Vec<usize> = Vec::new();
        let mut collapsed_depth: Option<usize> = None;

        for (i, task) in flat.iter().enumerate() {
            if let Some(depth) = collapsed_depth {
                if task.indent_level > depth {
                    continue;
                }
                collapsed_depth = None;
            }

            if !subtree_match[i] {
                continue;
            }

            visible_indices.push(i);

            let has_subtasks = !task.subtasks.is_empty();
            let is_expanded = self.expanded.contains(&i);
            if has_subtasks && !is_expanded {
                collapsed_depth = Some(task.indent_level);
            }
        }

        if self.sort.is_some() {
            let sorter = self.sort.unwrap().to_sorter(self.sort_desc);
            let top_level: Vec<usize> = visible_indices
                .iter()
                .copied()
                .filter(|&i| flat[i].indent_level == 0)
                .collect();
            let mut sorted_groups: Vec<(usize, Vec<usize>)> = Vec::new();
            for &tl in &top_level {
                let mut group = vec![tl];
                let tl_depth = flat[tl].indent_level;
                let pos = visible_indices.iter().position(|&x| x == tl).unwrap();
                for &idx in &visible_indices[pos + 1..] {
                    if flat[idx].indent_level <= tl_depth {
                        break;
                    }
                    group.push(idx);
                }
                sorted_groups.push((tl, group));
            }
            sorted_groups.sort_by(|&(a, _), &(b, _)| sorter(&flat[a], &flat[b]));
            visible_indices = sorted_groups
                .into_iter()
                .flat_map(|(_, group)| group)
                .collect();
        }

        visible_indices
            .into_iter()
            .map(|i| {
                let has_subtasks = !flat[i].subtasks.is_empty();
                let is_expanded = self.expanded.contains(&i);
                (i, has_subtasks && is_expanded)
            })
            .collect()
    }

    fn refresh_item_sizes(&mut self) {
        let tasks = self.visible_tasks();
        self.item_sizes = Rc::new(
            tasks
                .iter()
                .map(|_| Size {
                    width: px(0.0),
                    height: px(TODO_ROW_HEIGHT),
                })
                .collect(),
        );
    }

    fn toggle_complete(&mut self, index: usize, cx: &mut Context<Self>) {
        let flat = self.todo.list();
        if let Some(task) = flat.get(index) {
            if task.completed {
                let _ = self.todo.unmark([index as i64]);
            } else {
                let _ = self.todo.mark([index as i64]);
            }
        }
        self.refresh_item_sizes();
        cx.notify();
    }

    fn toggle_expand(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.expanded.contains(&index) {
            self.expanded.remove(&index);
        } else {
            self.expanded.insert(index);
        }
        self.refresh_item_sizes();
        cx.notify();
    }

    fn add_subtask(&mut self, parent_index: usize, cx: &mut Context<Self>) {
        let flat = self.todo.list();
        if let Some(parent) = flat.get(parent_index) {
            let indent = " ".repeat(parent.indent_level + 1);
            let today = today_string();
            let raw = format!("{indent}{today} New subtask");
            let insert_at = parent_index + 1;
            if let Err(e) = self.todo.insert(insert_at as i64, raw) {
                self.parse_errors.push(format!("Add subtask failed: {e}"));
            }
            let _ = self.todo.save(None);
            self.shift_expanded_after_insert(insert_at);
            self.expanded.insert(parent_index);
            if let Some(sel) = self.selected {
                if sel >= insert_at {
                    self.selected = Some(sel + 1);
                }
            }
            self.clear_row_inputs();
            self.pending_focus_desc = Some(insert_at);
        }
        self.refresh_item_sizes();
        cx.notify();
    }

    fn delete_task(&mut self, index: usize, cx: &mut Context<Self>) {
        let old_len = self.todo.list().len();
        let _ = self.todo.remove([index as i64]);
        let new_len = self.todo.list().len();
        let removed = old_len.saturating_sub(new_len);

        let old = std::mem::take(&mut self.expanded);
        for idx in old {
            if idx < index {
                self.expanded.insert(idx);
            } else if idx >= index + removed {
                self.expanded.insert(idx - removed);
            }
        }

        self.clear_row_inputs();
        if index > 0 && self.todo.list().len() > 0 {
            self.pending_focus_desc = Some(index - 1);
        } else if self.todo.list().is_empty() {
            self.pending_focus_search = true;
        } else {
            self.pending_focus_desc = Some(0);
        }
        if let Some(sel) = self.selected {
            let len = self.todo.list().len();
            if sel >= len && len > 0 {
                self.selected = Some(len - 1);
            } else if sel >= index + removed {
                self.selected = Some(sel - removed);
            }
        }
        self.refresh_item_sizes();
        cx.notify();
    }

    fn shift_expanded_after_insert(&mut self, at: usize) {
        let old = std::mem::take(&mut self.expanded);
        for idx in old {
            if idx >= at {
                self.expanded.insert(idx + 1);
            } else {
                self.expanded.insert(idx);
            }
        }
    }

    fn clear_row_inputs(&mut self) {
        self.desc_inputs.clear();
        self.date_inputs.clear();
        self.desc_subs.clear();
        self.date_subs.clear();
    }

    fn update_task_field(&mut self, index: usize, patch: TaskPatch, cx: &mut Context<Self>) {
        if let Err(e) = self.todo.update(index as i64, patch) {
            self.parse_errors.push(format!("Save failed: {e}"));
        }
        let _ = self.todo.save(None);
        self.priority_picker_open = None;
        self.refresh_item_sizes();
        cx.notify();
    }

    fn commit_description(&mut self, index: usize, value: &str, cx: &mut Context<Self>) {
        let flat = self.todo.list();
        if let Some(task) = flat.get(index) {
            let mut prefix_parts: Vec<String> = Vec::new();
            if task.completed {
                prefix_parts.push("x".to_string());
                if let Some(d) = task.completion_date {
                    prefix_parts.push(d.to_string());
                }
            }
            if let Some(p) = task.priority {
                prefix_parts.push(format!("({})", p.as_char()));
            }
            if let Some(d) = task.creation_date {
                prefix_parts.push(d.to_string());
            }
            prefix_parts.push(value.to_string());
            let raw_line = prefix_parts.join(" ");

            let parser = TodoTxtParser::new();
            match parser.parse_line(&raw_line) {
                Ok(parsed) => {
                    self.update_task_field(
                        index,
                        TaskPatch {
                            raw: Some(raw_line),
                            description: Some(parsed.description),
                            projects: Some(parsed.projects),
                            contexts: Some(parsed.contexts),
                            extensions: Some(parsed.extensions),
                            ..Default::default()
                        },
                        cx,
                    );
                }
                Err(_) => {
                    self.update_task_field(
                        index,
                        TaskPatch {
                            raw: Some(raw_line),
                            description: Some(value.to_string()),
                            ..Default::default()
                        },
                        cx,
                    );
                }
            }
        } else {
            self.update_task_field(
                index,
                TaskPatch {
                    description: Some(value.to_string()),
                    ..Default::default()
                },
                cx,
            );
        }
    }

    fn commit_date(&mut self, index: usize, value: &str, cx: &mut Context<Self>) {
        let date = parse_date(value);
        let flat = self.todo.list();
        if let Some(task) = flat.get(index) {
            let mut prefix_parts: Vec<String> = Vec::new();
            if task.completed {
                prefix_parts.push("x".to_string());
                if let Some(d) = task.completion_date {
                    prefix_parts.push(d.to_string());
                }
            }
            if let Some(p) = task.priority {
                prefix_parts.push(format!("({})", p.as_char()));
            }
            if let Some(d) = date {
                prefix_parts.push(d.to_string());
            } else if let Some(d) = task.creation_date {
                prefix_parts.push(d.to_string());
            }
            prefix_parts.push(task.description.clone());
            let raw_line = prefix_parts.join(" ");

            let parser = TodoTxtParser::new();
            if let Ok(parsed) = parser.parse_line(&raw_line) {
                self.update_task_field(
                    index,
                    TaskPatch {
                        raw: Some(raw_line),
                        creation_date: Some(date),
                        description: Some(parsed.description),
                        projects: Some(parsed.projects),
                        contexts: Some(parsed.contexts),
                        extensions: Some(parsed.extensions),
                        ..Default::default()
                    },
                    cx,
                );
            } else {
                self.update_task_field(
                    index,
                    TaskPatch {
                        creation_date: Some(date),
                        ..Default::default()
                    },
                    cx,
                );
            }
        } else {
            self.update_task_field(
                index,
                TaskPatch {
                    creation_date: Some(date),
                    ..Default::default()
                },
                cx,
            );
        }
    }

    fn commit_priority(&mut self, index: usize, value: &str, cx: &mut Context<Self>) {
        let priority = if value.is_empty() {
            None
        } else {
            Priority::from_token(value).ok()
        };
        let flat = self.todo.list();
        if let Some(task) = flat.get(index) {
            let mut prefix_parts: Vec<String> = Vec::new();
            if task.completed {
                prefix_parts.push("x".to_string());
                if let Some(d) = task.completion_date {
                    prefix_parts.push(d.to_string());
                }
            }
            if let Some(p) = priority {
                prefix_parts.push(format!("({})", p.as_char()));
            }
            if let Some(d) = task.creation_date {
                prefix_parts.push(d.to_string());
            }
            prefix_parts.push(task.description.clone());
            let raw_line = prefix_parts.join(" ");

            let parser = TodoTxtParser::new();
            if let Ok(parsed) = parser.parse_line(&raw_line) {
                self.update_task_field(
                    index,
                    TaskPatch {
                        raw: Some(raw_line),
                        priority: Some(priority),
                        description: Some(parsed.description),
                        projects: Some(parsed.projects),
                        contexts: Some(parsed.contexts),
                        extensions: Some(parsed.extensions),
                        ..Default::default()
                    },
                    cx,
                );
            } else {
                self.update_task_field(
                    index,
                    TaskPatch {
                        priority: Some(priority),
                        ..Default::default()
                    },
                    cx,
                );
            }
        } else {
            self.update_task_field(
                index,
                TaskPatch {
                    priority: Some(priority),
                    ..Default::default()
                },
                cx,
            );
        }
    }

    fn ensure_row_inputs(
        &mut self,
        flat_index: usize,
        desc: &str,
        date_str: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.desc_inputs.contains_key(&flat_index) {
            let entity = cx.new(|cx| InputState::new(window, cx).default_value(desc.to_string()));
            let idx = flat_index;
            let sub = cx.subscribe_in(
                &entity,
                window,
                move |this, input, event: &InputEvent, _window, cx| {
                    if let InputEvent::Change = event {
                        let value = input.read(cx).value().to_string();
                        this.commit_description(idx, &value, cx);
                    }
                },
            );
            self.desc_subs.insert(flat_index, sub);
            self.desc_inputs.insert(flat_index, entity);
        }

        if !self.date_inputs.contains_key(&flat_index) {
            let entity = cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(date_str.to_string())
                    .placeholder("yyyy-mm-dd")
            });
            let idx = flat_index;
            let sub = cx.subscribe_in(
                &entity,
                window,
                move |this, input, event: &InputEvent, _window, cx| {
                    if let InputEvent::Change = event {
                        let value = input.read(cx).value().to_string();
                        if value.is_empty() || parse_date(&value).is_some() {
                            this.commit_date(idx, &value, cx);
                        }
                    }
                },
            );
            self.date_subs.insert(flat_index, sub);
            self.date_inputs.insert(flat_index, entity);
        }
    }
}

impl Render for TodoTxtState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visible = self.visible_tasks();

        let task_count = self.todo.list().len();
        let task_data: Vec<(String, Option<String>, bool)> = {
            let flat = self.todo.list();
            flat.iter()
                .map(|t| {
                    (
                        t.description.clone(),
                        t.creation_date.map(|d| d.to_string()),
                        t.completed,
                    )
                })
                .collect()
        };

        // Clean up stale entries
        self.desc_inputs.retain(|k, _| *k < task_count);
        self.date_inputs.retain(|k, _| *k < task_count);
        self.desc_subs.retain(|k, _| *k < task_count);
        self.date_subs.retain(|k, _| *k < task_count);

        // Ensure Input entities exist for all visible rows
        for &(flat_index, _) in &visible {
            if flat_index < task_data.len() {
                let (ref desc, ref date_str, _) = task_data[flat_index];
                let ds = date_str.clone().unwrap_or_default();
                self.ensure_row_inputs(flat_index, desc, &ds, window, cx);
            }
        }

        if let Some(focus_idx) = self.pending_focus_desc.take() {
            if let Some(e) = self.desc_inputs.get(&focus_idx) {
                e.focus_handle(cx).focus(window, cx);
            }
        }
        if self.pending_focus_search {
            self.pending_focus_search = false;
            self.search_input.focus_handle(cx).focus(window, cx);
        }

        let header = self.render_header(cx);
        let error_banner = self.render_error_banner(cx);
        let task_list = self.render_task_list(&visible, cx);
        let new_row = self.render_new_task_row(cx);

        let total = self.todo.list().len();
        let completed = self.todo.list().iter().filter(|t| t.completed).count();
        let progress = if total > 0 {
            completed as f32 / total as f32
        } else {
            0.0
        };

        v_flex()
            .size_full()
            .overflow_hidden()
            .child(header)
            .child(
                div()
                    .h(px(6.0))
                    .w_full()
                    .bg(cx.theme().border)
                    .child(div().h_full().w(relative(progress)).bg(cx.theme().success)),
            )
            .children(error_banner)
            .child(task_list)
            .child(new_row)
    }
}

impl TodoTxtState {
    fn render_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let search = Input::new(&self.search_input).cleanable(true);
        let filter_select = Select::new(&self.filter_select);
        let sort_select = Select::new(&self.sort_select);

        let sort_icon = if self.sort_desc {
            Icon::default()
                .path(SharedString::from(ARROW_DOWN_AZ_ICON))
                .size_4()
        } else {
            Icon::default()
                .path(SharedString::from(ARROW_UP_AZ_ICON))
                .size_4()
        };

        h_flex()
            .h(px(TODO_HEADER_HEIGHT))
            .w_full()
            .items_center()
            .gap_1()
            .px_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("todo-add-btn")
                    .ghost()
                    .icon(IconName::Plus)
                    .mr_1()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.new_task_input.focus_handle(cx).focus(window, cx);
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(100.0))
                    .child(search)
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        if event.keystroke.key == "down" {
                            let visible = this.visible_tasks();
                            if let Some(&(first_fi, _)) = visible.first() {
                                if let Some(e) = this.desc_inputs.get(&first_fi) {
                                    e.focus_handle(cx).focus(window, cx);
                                }
                            } else {
                                this.new_task_input.focus_handle(cx).focus(window, cx);
                            }
                        }
                    })),
            )
            .child(
                h_flex()
                    .items_center()
                    .ml_4()
                    .gap_0p5()
                    .flex_shrink_0()
                    .child(
                        Icon::default()
                            .path(SharedString::from(FUNNEL_ICON))
                            .size_4()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(div().w(px(120.0)).child(filter_select)),
            )
            .child(div().ml_1().flex_shrink_0().w(px(120.0)).child(sort_select))
            .child(
                Button::new("todo-sort-dir")
                    .ghost()
                    .icon(sort_icon)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.sort_desc = !this.sort_desc;
                        this.refresh_item_sizes();
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn render_error_banner(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.parse_errors.is_empty() {
            return None;
        }
        let count = self.parse_errors.len();
        Some(
            h_flex()
                .w_full()
                .px_3()
                .py_1()
                .bg(cx.theme().warning)
                .text_color(cx.theme().warning_foreground)
                .text_sm()
                .child(format!("{count} line(s) failed to parse"))
                .into_any_element(),
        )
    }

    fn render_task_list(
        &mut self,
        visible: &[(usize, bool)],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if visible.is_empty() {
            return v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap_2()
                .text_color(cx.theme().muted_foreground)
                .child(
                    Icon::new(IconName::Inbox)
                        .size_8()
                        .text_color(cx.theme().muted_foreground.opacity(0.4)),
                )
                .child(div().text_sm().child("No tasks to display"))
                .into_any_element();
        }

        let entity = cx.entity().clone();
        let sizes = self.item_sizes.clone();
        let selected = self.selected;
        let visible_owned = visible.to_vec();

        v_virtual_list(
            entity,
            "todo-task-list",
            sizes,
            move |state, range, _window, cx| {
                range
                    .map(|i| {
                        let (flat_index, _) = visible_owned[i];
                        let flat = state.todo.list();
                        let task = match flat.get(flat_index) {
                            Some(t) => t,
                            None => return div().h(px(TODO_ROW_HEIGHT)).into_any(),
                        };
                        let depth = task.indent_level;
                        let is_selected = selected == Some(i);
                        state.render_task_row(flat_index, task, depth, is_selected, cx)
                    })
                    .collect()
            },
        )
        .track_scroll(&self.scroll_handle)
        .flex_1()
        .into_any_element()
    }

    fn render_task_row(
        &self,
        flat_index: usize,
        task: &Task,
        depth: usize,
        is_selected: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let indent = px(depth as f32 * TODO_INDENT_PX);
        let has_subtasks = !task.subtasks.is_empty();
        let is_expanded = self.expanded.contains(&flat_index);

        let row_bg = if is_selected {
            cx.theme().accent.opacity(0.1)
        } else {
            gpui::transparent_black()
        };

        let mut row = h_flex()
            .id(ElementId::NamedInteger(
                "todo-row".into(),
                flat_index as u64,
            ))
            .h(px(TODO_ROW_HEIGHT))
            .w_full()
            .items_center()
            .gap_1()
            .pl(indent + px(8.0))
            .pr_3()
            .bg(row_bg)
            .border_b_1()
            .border_color(cx.theme().border.opacity(0.3))
            .cursor_pointer()
            .group("todo-row");

        // Expand/collapse arrow
        if has_subtasks {
            let fi = flat_index;
            let arrow_icon = if is_expanded {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            };
            row = row.child(
                Button::new(ElementId::NamedInteger("todo-expand".into(), fi as u64))
                    .ghost()
                    .xsmall()
                    .icon(arrow_icon)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_expand(fi, cx);
                    })),
            );
        } else {
            row = row.child(div().flex_shrink_0().w(px(TODO_COL_EXPAND)));
        }

        // Checkbox
        let completed = task.completed;
        let fi = flat_index;
        row = row.child(
            Checkbox::new(ElementId::NamedInteger("todo-check".into(), fi as u64))
                .checked(completed)
                .on_click(cx.listener(move |this, _checked, _window, cx| {
                    this.toggle_complete(fi, cx);
                })),
        );

        // Priority (popover)
        row = row.child(self.render_priority_picker(flat_index, cx));

        // Date
        if let Some(date_entity) = self.date_inputs.get(&flat_index) {
            let date_val = date_entity.read(cx).value().to_string();
            let is_valid = date_val.is_empty() || parse_date(&date_val).is_some();
            let date_color = if task.completed || is_valid {
                cx.theme().muted_foreground
            } else {
                cx.theme().danger
            };
            let date_input = Input::new(date_entity)
                .appearance(false)
                .p_0()
                .text_color(date_color);
            let fi = flat_index;
            row = row.child(
                div()
                    .id(ElementId::NamedInteger("todo-date-wrap".into(), fi as u64))
                    .flex_shrink_0()
                    .w(px(TODO_COL_DATE))
                    .items_center()
                    .text_sm()
                    .child(date_input)
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        if event.keystroke.key == "escape" {
                            if let Some(e) = this.date_inputs.get(&fi) {
                                let original = this
                                    .todo
                                    .list()
                                    .get(fi)
                                    .and_then(|t| t.creation_date)
                                    .map(|d| d.to_string())
                                    .unwrap_or_default();
                                e.update(cx, |s, cx| s.set_value(original, window, cx));
                            }
                        }
                    })),
            );
        } else {
            row = row.child(div().flex_shrink_0().w(px(TODO_COL_DATE)));
        }

        // Description — ALWAYS an Input
        if let Some(desc_entity) = self.desc_inputs.get(&flat_index) {
            let fi = flat_index;
            let mut desc_input = Input::new(desc_entity).appearance(false).p_0();
            if task.completed {
                desc_input = desc_input.text_color(cx.theme().muted_foreground);
            }
            row = row.child(
                div()
                    .id(ElementId::NamedInteger("todo-desc-wrap".into(), fi as u64))
                    .flex_1()
                    .items_center()
                    .child(desc_input)
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        match event.keystroke.key.as_str() {
                            "enter" if event.keystroke.modifiers.shift => {
                                this.add_subtask(fi, cx);
                            }
                            "enter" => {
                                this.toggle_complete(fi, cx);
                            }
                            "backspace"
                                if event.keystroke.modifiers.shift
                                    && event.keystroke.modifiers.secondary() =>
                            {
                                this.delete_task(fi, cx);
                            }
                            "up" => {
                                let visible = this.visible_tasks();
                                if let Some(pos) = visible.iter().position(|&(idx, _)| idx == fi) {
                                    if pos > 0 {
                                        let prev_fi = visible[pos - 1].0;
                                        if let Some(e) = this.desc_inputs.get(&prev_fi) {
                                            e.focus_handle(cx).focus(window, cx);
                                        }
                                    } else {
                                        this.search_input.focus_handle(cx).focus(window, cx);
                                    }
                                }
                            }
                            "down" => {
                                let visible = this.visible_tasks();
                                if let Some(pos) = visible.iter().position(|&(idx, _)| idx == fi) {
                                    if pos + 1 < visible.len() {
                                        let next_fi = visible[pos + 1].0;
                                        if let Some(e) = this.desc_inputs.get(&next_fi) {
                                            e.focus_handle(cx).focus(window, cx);
                                        }
                                    } else {
                                        this.new_task_input.focus_handle(cx).focus(window, cx);
                                    }
                                }
                            }
                            _ => {}
                        }
                    })),
            );
        } else {
            row = row.child(div().flex_1());
        }

        // Project pills
        for project in &task.projects {
            row = row.child(render_pill(
                &format!("+{project}"),
                if task.completed {
                    cx.theme().muted_foreground
                } else {
                    cx.theme().info
                },
            ));
        }

        // Context pills
        for context in &task.contexts {
            row = row.child(render_pill(
                &format!("@{context}"),
                if task.completed {
                    cx.theme().muted_foreground
                } else {
                    cx.theme().success
                },
            ));
        }

        // Hover actions
        let fi = flat_index;
        row = row.child(
            h_flex()
                .gap_1()
                .opacity(0.0)
                .flex_shrink_0()
                .when(true, |el| {
                    el.group_hover("todo-row", |style| style.opacity(1.0))
                })
                .child(
                    div()
                        .id(ElementId::NamedInteger("todo-addsub".into(), fi as u64))
                        .cursor_pointer()
                        .child(
                            Icon::new(IconName::Plus)
                                .size_3()
                                .text_color(cx.theme().muted_foreground),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.add_subtask(fi, cx);
                        })),
                )
                .child(
                    div()
                        .id(ElementId::NamedInteger("todo-del".into(), fi as u64))
                        .cursor_pointer()
                        .child(
                            Icon::new(IconName::Close)
                                .size_3()
                                .text_color(cx.theme().danger),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.delete_task(fi, cx);
                        })),
                ),
        );

        row.into_any()
    }

    fn render_priority_picker(&self, flat_index: usize, cx: &mut Context<Self>) -> AnyElement {
        let entity = cx.entity();

        let trigger = PriorityTrigger {
            id: ElementId::NamedInteger("pri-trigger".into(), flat_index as u64),
            editor: entity.clone(),
            task_index: flat_index,
            current: self.todo.list().get(flat_index).and_then(|t| t.priority),
            completed: self
                .todo
                .list()
                .get(flat_index)
                .map_or(false, |t| t.completed),
        };

        let content_entity = entity.clone();
        let content =
            move |_: &mut PopoverState, _window: &mut Window, cx: &mut Context<PopoverState>| {
                let options: [(Option<Priority>, &str); 6] = [
                    (None, "×"),
                    (Some(Priority('A')), "A"),
                    (Some(Priority('B')), "B"),
                    (Some(Priority('C')), "C"),
                    (Some(Priority('D')), "D"),
                    (Some(Priority('E')), "E"),
                ];
                let mut menu = v_flex().py_1().min_w(px(80.0));

                for (pri, label) in &options {
                    let color = match pri {
                        Some(p) => priority_color(p.as_char()),
                        None => cx.theme().muted_foreground,
                    };
                    let pri_char = pri.map(|p| p.as_char());
                    let ent = content_entity.clone();
                    menu = menu.child(
                        div()
                            .id(ElementId::NamedInteger(
                                "pri-opt".into(),
                                (flat_index as u64) << 4 | (pri_char.unwrap_or('×') as u64),
                            ))
                            .px_3()
                            .py_1()
                            .cursor_pointer()
                            .hover(|s| s.bg(cx.theme().accent.opacity(0.1)))
                            .text_sm()
                            .text_color(color)
                            .child(*label)
                            .on_click(move |_, window, app| {
                                let value = match pri_char {
                                    Some(c) => format!("({c})"),
                                    None => String::new(),
                                };
                                let _ = app.update_entity(&ent, |this: &mut TodoTxtState, cx| {
                                    this.commit_priority(flat_index, &value, cx);
                                    this.priority_picker_open = None;
                                });
                                window.refresh();
                            }),
                    );
                }

                menu.into_any_element()
            };

        Popover::new(ElementId::NamedInteger("pri-pop".into(), flat_index as u64))
            .open(self.priority_picker_open == Some(flat_index))
            .on_open_change({
                let ent = entity.clone();
                move |open: &bool, _window: &mut Window, app: &mut App| {
                    let _ = app.update_entity(&ent, |this: &mut TodoTxtState, _cx| {
                        if *open {
                            this.priority_picker_open = Some(flat_index);
                        } else if this.priority_picker_open == Some(flat_index) {
                            this.priority_picker_open = None;
                        }
                    });
                }
            })
            .trigger(trigger)
            .content(content)
            .into_any_element()
    }

    fn render_new_task_row(&self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .h(px(TODO_NEW_ROW_HEIGHT))
            .w_full()
            .items_center()
            .gap_2()
            .px_3()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(div().w(px(TODO_INDENT_PX + 16.0)))
            .child(
                div()
                    .flex_1()
                    .id("new-task-input-wrap")
                    .child(Input::new(&self.new_task_input).appearance(false))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        match event.keystroke.key.as_str() {
                            "enter" if !event.keystroke.modifiers.secondary() => {
                                let value = this.new_task_input.read(cx).value();
                                let trimmed = value.trim().to_string();
                                if !trimmed.is_empty() {
                                    let today = today_string();
                                    let line = format!("{today} {trimmed}");
                                    if let Err(e) = this.todo.add(line) {
                                        this.parse_errors.push(format!("Add task failed: {e}"));
                                    }
                                    let _ = this.todo.save(None);
                                    this.refresh_item_sizes();
                                    cx.notify();
                                }
                                this.new_task_input.update(cx, |state, cx| {
                                    state.set_value("", window, cx);
                                });
                            }
                            "up" => {
                                let visible = this.visible_tasks();
                                if let Some(&(last_fi, _)) = visible.last() {
                                    if let Some(e) = this.desc_inputs.get(&last_fi) {
                                        e.focus_handle(cx).focus(window, cx);
                                    }
                                } else {
                                    this.search_input.focus_handle(cx).focus(window, cx);
                                }
                            }
                            _ => {}
                        }
                    })),
            )
            .into_any_element()
    }
}

// --- Helpers

fn today_string() -> String {
    let now = time::OffsetDateTime::now_utc();
    now.date().to_string()
}

fn matches_task(task: &Task, query: &str) -> bool {
    if task.description.to_lowercase().contains(query) {
        return true;
    }
    if let Some(ref p) = task.priority {
        if p.to_string().to_lowercase().contains(query) {
            return true;
        }
    }
    if let Some(d) = task.creation_date {
        if d.to_string().to_lowercase().contains(query) {
            return true;
        }
    }
    for proj in &task.projects {
        if proj.to_lowercase().contains(query) {
            return true;
        }
    }
    for ctx in &task.contexts {
        if ctx.to_lowercase().contains(query) {
            return true;
        }
    }
    for (key, val) in &task.extensions {
        if key.to_lowercase().contains(query) || val.to_string().to_lowercase().contains(query) {
            return true;
        }
    }
    false
}

fn priority_color(c: char) -> Hsla {
    match c {
        'A' => rgb(0xEF4444).into(),
        'B' => rgb(0xF97316).into(),
        'C' => rgb(0xEAB308).into(),
        'D' => rgb(0x22C55E).into(),
        'E' => rgb(0x3B82F6).into(),
        _ => rgb(0x9CA3AF).into(),
    }
}

fn parse_date(s: &str) -> Option<time::Date> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let year: i32 = parts[0].parse().ok()?;
    let month: u8 = parts[1].parse().ok()?;
    let day: u8 = parts[2].parse().ok()?;
    let month = time::Month::try_from(month).ok()?;
    time::Date::from_calendar_date(year, month, day).ok()
}

fn render_pill(label: &str, color: Hsla) -> AnyElement {
    div()
        .h(px(TODO_COL_CHECK))
        .flex()
        .items_center()
        .px(px(TODO_PILL_PADDING_H))
        .rounded(px(TODO_PILL_RADIUS))
        .bg(color.opacity(0.1))
        .text_color(color)
        .text_sm()
        .child(label.to_string())
        .into_any_element()
}
