use std::ops::Div;

use gpui::{
    AnyElement, Context, Element, Focusable, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Render, Styled, Window, div, px, relative,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::Input;
use gpui_component::select::Select;
use gpui_component::{ActiveTheme, Icon, IconName, h_flex, v_flex, v_virtual_list};

use conv::ConvAsUtil;

use crate::ui::icons::DatalithIcon;

use super::TodoTxtState;
use super::constants::{TODO_HEADER_HEIGHT, TODO_INDENT_PX, TODO_NEW_ROW_HEIGHT, TODO_ROW_HEIGHT};

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
            if let Some((desc, date_str, _)) = task_data.get(flat_index) {
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
        let progress: f32 = if total > 0 {
            completed
                .approx()
                .unwrap_or(0.0)
                .div(total.approx().unwrap_or(f32::INFINITY))
        } else {
            0.0
        };

        v_flex()
            .size_full()
            .overflow_hidden()
            .track_focus(&self.editor_focus)
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
    fn render_header(&self, cx: &Context<Self>) -> AnyElement {
        let search = Input::new(&self.search_input).cleanable(true);
        let filter_select = Select::new(&self.filter_select);
        let sort_select = Select::new(&self.sort_select);

        let sort_icon = if self.workspace.sort_descending() {
            Icon::new(DatalithIcon::ArrowDownAz).size_4()
        } else {
            Icon::new(DatalithIcon::ArrowUpAz).size_4()
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
                        Icon::new(DatalithIcon::Funnel)
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

    fn render_error_banner(&self, cx: &Context<Self>) -> Option<AnyElement> {
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

    fn render_task_list(&self, visible: &[(usize, bool)], cx: &Context<Self>) -> AnyElement {
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

        let entity = cx.entity();
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
                        let Some(&(flat_index, _)) = visible_owned.get(i) else {
                            return div().h(px(TODO_ROW_HEIGHT)).into_any();
                        };
                        let Some(task) = state.workspace.task(flat_index) else {
                            return div().h(px(TODO_ROW_HEIGHT)).into_any();
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

    fn render_new_task_row(&self, cx: &Context<Self>) -> AnyElement {
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
