use std::ops::{Add, Mul};

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, AppContext, Context, Element, ElementId, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::checkbox::Checkbox;
use gpui_component::input::Input;
use gpui_component::popover::{Popover, PopoverState};
use gpui_component::{ActiveTheme, Icon, IconName, Sizable, h_flex, v_flex};

use conv::{ConvUtil, UnwrapOrInf, UnwrapOrSaturate, ValueFrom};
use txtodo::{Priority, Task};

use crate::document::todo_txt::parse_date;

use super::TodoTxtState;
use super::constants::{TODO_COL_DATE, TODO_COL_EXPAND, TODO_INDENT_PX, TODO_ROW_HEIGHT};
use super::priority::{PRIORITY_VALUES, PriorityTrigger};

fn element_id(name: &'static str, index: usize) -> ElementId {
    ElementId::NamedInteger(name.into(), u64::value_from(index).unwrap_or_saturate())
}

fn priority_option_id(flat_index: usize, pri_char: Option<char>) -> u64 {
    (u64::value_from(flat_index).unwrap_or_saturate()) << 4
        | u64::from(u32::from(pri_char.unwrap_or('×')))
}

impl TodoTxtState {
    pub(super) fn render_task_row(
        &self,
        flat_index: usize,
        task: &Task,
        depth: usize,
        is_selected: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let indent = (depth.approx_as::<f32>().unwrap_or_inf()).mul(TODO_INDENT_PX);
        let has_subtasks = !task.subtasks.is_empty();
        let is_expanded = self.workspace.is_expanded(flat_index);

        let row_bg = if is_selected {
            cx.theme().accent.opacity(0.1)
        } else {
            gpui::transparent_black()
        };

        let mut row = h_flex()
            .id(element_id("todo-row", flat_index))
            .h(px(TODO_ROW_HEIGHT))
            .w_full()
            .items_center()
            .gap_1()
            .pl(px(indent.add(8.0)))
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
                        Button::new(element_id("todo-expand", fi))
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
            Checkbox::new(element_id("todo-check", fi))
                .checked(completed)
                .on_click(cx.listener(move |this, _checked, window, cx| {
                    this.toggle_complete(fi, window, cx);
                })),
        );

        // Priority (popover)
        row = row.child(self.render_priority_picker(flat_index, cx));

        // Date
        row = row.child(self.render_date_cell(flat_index, task, cx));

        // Description — ALWAYS an Input
        row = row.child(self.render_description_cell(flat_index, task, cx));

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
        row = row.child(Self::render_hover_actions(flat_index, cx));

        row.into_any()
    }

    fn render_date_cell(&self, flat_index: usize, task: &Task, cx: &Context<Self>) -> AnyElement {
        let Some(date_entity) = self.date_inputs.get(&flat_index) else {
            return div()
                .flex_shrink_0()
                .w(px(TODO_COL_DATE))
                .into_any_element();
        };
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
        div()
            .id(element_id("todo-date-wrap", fi))
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
            }))
            .into_any_element()
    }

    fn render_description_cell(
        &self,
        flat_index: usize,
        task: &Task,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(desc_entity) = self.desc_inputs.get(&flat_index) else {
            return div().flex_1().into_any_element();
        };
        let fi = flat_index;
        let mut desc_input = Input::new(desc_entity).appearance(false).p_0();
        if task.completed {
            desc_input = desc_input.text_color(cx.theme().muted_foreground);
        }
        div()
            .id(element_id("todo-desc-wrap", fi))
            .flex_1()
            .items_center()
            .child(desc_input)
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "enter" if event.keystroke.modifiers.shift => {
                        this.add_subtask(fi, cx);
                    }
                    "enter" => {
                        this.toggle_complete(fi, window, cx);
                    }
                    "backspace"
                        if event.keystroke.modifiers.shift
                            && event.keystroke.modifiers.secondary() =>
                    {
                        this.delete_task(fi, window, cx);
                    }
                    "up" => {
                        let visible = this.workspace.visible_tasks();
                        if let Some(pos) = visible.iter().position(|&(idx, _)| idx == fi) {
                            if let Some(&(prev_fi, _)) =
                                pos.checked_sub(1).and_then(|p| visible.get(p))
                            {
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
                            if let Some(&(next_fi, _)) =
                                pos.checked_add(1).and_then(|p| visible.get(p))
                            {
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
            }))
            .into_any_element()
    }

    fn render_hover_actions(flat_index: usize, cx: &Context<Self>) -> AnyElement {
        let fi = flat_index;
        h_flex()
            .gap_1()
            .opacity(0.0)
            .flex_shrink_0()
            .when(true, |el| {
                el.group_hover("todo-row", |style| style.opacity(1.0))
            })
            .child(
                div()
                    .id(element_id("todo-addsub", fi))
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
                    .id(element_id("todo-del", fi))
                    .cursor_pointer()
                    .child(
                        Icon::new(IconName::Close)
                            .size_3()
                            .text_color(cx.theme().danger),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.delete_task(fi, window, cx);
                    })),
            )
            .into_any_element()
    }

    pub(super) fn render_priority_picker(
        &self,
        flat_index: usize,
        cx: &Context<Self>,
    ) -> AnyElement {
        let entity = cx.entity();

        let trigger = PriorityTrigger {
            id: element_id("pri-trigger", flat_index),
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
                    let color = pri.map_or_else(
                        || cx.theme().muted_foreground,
                        |p| super::priority_color(p.as_char()),
                    );
                    let pri_char = pri.map(Priority::as_char);
                    let ent = content_entity.clone();
                    menu = menu.child(
                        div()
                            .id(ElementId::NamedInteger(
                                "pri-opt".into(),
                                priority_option_id(flat_index, pri_char),
                            ))
                            .px_3()
                            .py_1()
                            .cursor_pointer()
                            .hover(|s| s.bg(cx.theme().accent.opacity(0.1)))
                            .text_sm()
                            .text_color(color)
                            .child(label)
                            .on_click(move |_, window, app| {
                                let value = pri_char.map_or_else(String::new, |c| format!("({c})"));
                                app.update_entity(&ent, |this: &mut Self, cx| {
                                    this.commit_priority(flat_index, &value, cx);
                                    this.priority_picker_open = None;
                                });
                                window.refresh();
                            }),
                    );
                }

                menu.into_any_element()
            };

        Popover::new(element_id("pri-pop", flat_index))
            .open(self.priority_picker_open == Some(flat_index))
            .on_open_change({
                move |open: &bool, _window: &mut Window, app: &mut App| {
                    app.update_entity(&entity, |this: &mut Self, _cx| {
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
