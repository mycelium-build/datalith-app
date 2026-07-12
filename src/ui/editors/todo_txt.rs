use std::collections::HashMap;
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

use txtodo::{Priority, Task};

use crate::app::assets::{ARROW_DOWN_AZ_ICON, ARROW_UP_AZ_ICON, FUNNEL_ICON};
use crate::document::handler::{FileHandler, ReloadOutcome};
use crate::document::todo_txt::{FilterKind, FocusTarget, SortKind, TodoTxtWorkspace, parse_date};

const TODO_ROW_HEIGHT: f32 = 32.0;
const TODO_NEW_ROW_HEIGHT: f32 = 36.0;

const TODO_HEADER_HEIGHT: f32 = 40.0;

const TODO_PILL_PADDING_H: f32 = 6.0;
const TODO_PILL_RADIUS: f32 = 4.0;

const TODO_INDENT_PX: f32 = 20.0;

const TODO_COL_EXPAND: f32 = 24.0;
const TODO_COL_CHECK: f32 = 24.0;
const TODO_COL_PRIORITY: f32 = 56.0;
const TODO_COL_DATE: f32 = 90.0;

const PRIORITY_VALUES: [Option<char>; 6] =
    [None, Some('A'), Some('B'), Some('C'), Some('D'), Some('E')];

pub(crate) fn reload_todo_txt(
    _path: &Path,
    handler: &mut FileHandler,
    _window: &mut Window,
    cx: &mut Context<FileHandler>,
) -> anyhow::Result<ReloadOutcome> {
    let Some(crate::ui::editors::EditorKind::TodoTxt(editor)) = handler.editor.as_ref() else {
        return Ok(ReloadOutcome::Unsupported);
    };
    editor.reload_from_disk(cx)
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
                app.update_entity(&editor, |this, cx| {
                    let current = this.workspace.task(task_index).and_then(|t| t.priority);
                    let current_idx = current
                        .and_then(|p| {
                            PRIORITY_VALUES
                                .iter()
                                .position(|&item| item == Some(p.as_char()))
                        })
                        .unwrap_or(0);
                    let new_idx = if key == "up" {
                        current_idx
                            .checked_sub(1)
                            .unwrap_or(PRIORITY_VALUES.len() - 1)
                    } else {
                        (current_idx + 1) % PRIORITY_VALUES.len()
                    };
                    let value = PRIORITY_VALUES[new_idx]
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
        let workspace = TodoTxtWorkspace::open(path);
        let total = workspace.task_count();

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
                        this.workspace
                            .set_search_query(this.search_input.read(cx).value().to_string());
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
                        this.workspace
                            .set_filter(FilterKind::from_index(index_path.row));
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
                        this.workspace
                            .set_sort(SortKind::from_index(index_path.row));
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
                        this.add_task(window, cx);
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
                workspace,
                priority_picker_open: None,
                pending_focus_desc: None,
                pending_focus_search: false,
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

    pub(crate) fn reload_from_disk(&self, cx: &mut App) -> anyhow::Result<ReloadOutcome> {
        self.state
            .update(cx, |state, cx| state.reload_from_disk(cx))
    }
}

// --- TodoTxtState

pub(crate) struct TodoTxtState {
    workspace: TodoTxtWorkspace,
    priority_picker_open: Option<usize>,
    pending_focus_desc: Option<usize>,
    pending_focus_search: bool,
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
    fn reload_from_disk(&mut self, cx: &mut Context<Self>) -> anyhow::Result<ReloadOutcome> {
        let outcome = self.workspace.reload_from_disk()?;
        if outcome == ReloadOutcome::Reloaded {
            self.clear_row_inputs();
            self.refresh_item_sizes();
            cx.notify();
        }
        Ok(outcome)
    }

    fn add_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.new_task_input.read(cx).value();
        if !value.trim().is_empty() {
            self.workspace.add_task(&value);
            self.clear_row_inputs();
            self.refresh_item_sizes();
            cx.notify();
        }
        self.new_task_input
            .update(cx, |state, cx| state.set_value("", window, cx));
    }

    fn refresh_item_sizes(&mut self) {
        // necessary because row count change and v_virtual_list GPUI components need a size for each rows (that can be different)
        let tasks = self.workspace.visible_tasks();
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
        self.workspace.toggle_complete(index);
        self.refresh_item_sizes();
        cx.notify();
    }

    fn toggle_expand(&mut self, index: usize, cx: &mut Context<Self>) {
        self.workspace.toggle_expanded(index);
        self.refresh_item_sizes();
        cx.notify();
    }

    fn add_subtask(&mut self, parent_index: usize, cx: &mut Context<Self>) {
        let outcome = self.workspace.add_subtask(parent_index);
        if let Some(FocusTarget::Task(index)) = outcome.focus {
            self.clear_row_inputs();
            self.pending_focus_desc = Some(index);
        }
        self.refresh_item_sizes();
        cx.notify();
    }

    fn delete_task(&mut self, index: usize, cx: &mut Context<Self>) {
        let outcome = self.workspace.delete_task(index);
        self.clear_row_inputs();
        match outcome.focus {
            Some(FocusTarget::Task(index)) => self.pending_focus_desc = Some(index),
            Some(FocusTarget::Search) => self.pending_focus_search = true,
            None => {}
        }
        self.refresh_item_sizes();
        cx.notify();
    }

    fn clear_row_inputs(&mut self) {
        self.desc_inputs.clear();
        self.date_inputs.clear();
        self.desc_subs.clear();
        self.date_subs.clear();
    }

    fn commit_description(&mut self, index: usize, value: &str, cx: &mut Context<Self>) {
        self.workspace.update_description(index, value);
        self.refresh_after_update(cx);
    }

    fn commit_date(&mut self, index: usize, value: &str, cx: &mut Context<Self>) {
        self.workspace.update_date(index, value);
        self.refresh_after_update(cx);
    }

    fn commit_priority(&mut self, index: usize, value: &str, cx: &mut Context<Self>) {
        self.workspace.update_priority(index, value);
        self.refresh_after_update(cx);
    }

    fn refresh_after_update(&mut self, cx: &mut Context<Self>) {
        self.priority_picker_open = None;
        self.refresh_item_sizes();
        cx.notify();
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
        let visible = self.workspace.visible_tasks();

        let task_count = self.workspace.task_count();
        let task_data: Vec<(String, Option<String>, bool)> = {
            let flat = self.workspace.tasks();
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

        if let Some(focus_idx) = self.pending_focus_desc.take()
            && let Some(e) = self.desc_inputs.get(&focus_idx)
        {
            e.focus_handle(cx).focus(window, cx);
        }
        if self.pending_focus_search {
            self.pending_focus_search = false;
            self.search_input.focus_handle(cx).focus(window, cx);
        }

        let header = self.render_header(cx);
        let error_banner = self.render_error_banner(cx);
        let task_list = self.render_task_list(&visible, cx);
        let new_row = self.render_new_task_row(cx);

        let total = self.workspace.task_count();
        let completed = self.workspace.completed_count();
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

        let sort_icon = if self.workspace.sort_descending() {
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
                            let visible = this.workspace.visible_tasks();
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
                        this.workspace.toggle_sort_direction();
                        this.refresh_item_sizes();
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn render_error_banner(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.workspace.parse_error_count() == 0 {
            return None;
        }
        let count = self.workspace.parse_error_count();
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
        let selected = self.workspace.selected();
        let visible_owned = visible.to_vec();

        v_virtual_list(
            entity,
            "todo-task-list",
            sizes,
            move |state, range, _window, cx| {
                range
                    .map(|i| {
                        let (flat_index, _) = visible_owned[i];
                        let task = match state.workspace.task(flat_index) {
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
        let is_expanded = self.workspace.is_expanded(flat_index);

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
                div()
                    .flex_shrink_0()
                    .w(px(TODO_COL_EXPAND))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Button::new(ElementId::NamedInteger("todo-expand".into(), fi as u64))
                            .ghost()
                            .xsmall()
                            .icon(arrow_icon)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_expand(fi, cx);
                            })),
                    ),
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
                        if event.keystroke.key == "escape"
                            && let Some(e) = this.date_inputs.get(&fi)
                        {
                            let original = this
                                .workspace
                                .task(fi)
                                .and_then(|t| t.creation_date)
                                .map(|d| d.to_string())
                                .unwrap_or_default();
                            e.update(cx, |s, cx| s.set_value(original, window, cx));
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
                                let visible = this.workspace.visible_tasks();
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
                                let visible = this.workspace.visible_tasks();
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
            current: self.workspace.task(flat_index).and_then(|t| t.priority),
            completed: self.workspace.task(flat_index).is_some_and(|t| t.completed),
        };

        let content_entity = entity.clone();
        let content =
            move |_: &mut PopoverState, _window: &mut Window, cx: &mut Context<PopoverState>| {
                let mut menu = v_flex().py_1().min_w(px(80.0));

                for value in PRIORITY_VALUES {
                    let pri = value.map(Priority);
                    let label = value.map_or_else(|| "×".to_string(), |value| value.to_string());
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
                            .child(label)
                            .on_click(move |_, window, app| {
                                let value = match pri_char {
                                    Some(c) => format!("({c})"),
                                    None => String::new(),
                                };
                                app.update_entity(&ent, |this: &mut TodoTxtState, cx| {
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
                    app.update_entity(&ent, |this: &mut TodoTxtState, _cx| {
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
                                this.add_task(window, cx);
                            }
                            "up" => {
                                let visible = this.workspace.visible_tasks();
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
