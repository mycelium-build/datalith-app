use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::checkbox::Checkbox;
use gpui_component::input::Input;
use gpui_component::popover::{Popover, PopoverState};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};

use txtodo::{Priority, Task};

use crate::document::todo_txt::parse_date;

use super::TodoTxtState;
use super::constants::*;
use super::priority::{PRIORITY_VALUES, PriorityTrigger};

impl TodoTxtState {
    pub(super) fn render_task_row(
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
            row = row.child(super::render_pill(
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
            row = row.child(super::render_pill(
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

    pub(super) fn render_priority_picker(
        &self,
        flat_index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
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
                        Some(p) => super::priority_color(p.as_char()),
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
}
