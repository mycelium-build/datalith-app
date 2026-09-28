use std::ops::Div;
use std::rc::Rc;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::scroll::Scrollbar;
use gpui_kit::component::select::Select;
use gpui_kit::component::{Icon, IconName, IndexPath, h_flex, v_flex, v_virtual_list};
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, DismissEvent, Element, ElementId, Entity, Focusable,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render, RenderOnce, Styled,
    Window, div, px, relative,
};

use conv::ConvAsUtil;

use crate::ui::icons::DatalithIcon;

use super::TodoTxtState;
use super::constants::{TODO_HEADER_HEIGHT, TODO_INDENT_PX, TODO_NEW_ROW_HEIGHT, TODO_ROW_HEIGHT};
use super::{FilterKind, SortKind};

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
            .bg(self.theme(cx).background)
            .text_color(self.theme(cx).foreground)
            .font_family(self.theme(cx).font_family.clone())
            .min_h_0()
            .overflow_hidden()
            .track_focus(&self.editor_focus)
            .child(header)
            .child(
                div().h(px(6.0)).w_full().bg(self.theme(cx).border).child(
                    div()
                        .h_full()
                        .w(relative(progress))
                        .bg(self.theme(cx).success),
                ),
            )
            .children(error_banner)
            .child(task_list)
            .child(new_row)
    }
}

impl TodoTxtState {
    fn render_header(&self, cx: &Context<Self>) -> AnyElement {
        let search = Input::new(&self.search_input)
            .cleanable(true)
            .bg(self.theme(cx).background)
            .text_color(self.theme(cx).foreground);
        let filter_select = self.render_select(PreviewSelectKind::Filter, cx);
        let sort_select = self.render_select(PreviewSelectKind::Sort, cx);

        let sort_icon = if self.workspace.sort_descending() {
            Icon::new(DatalithIcon::ArrowDownAz).size_4()
        } else {
            Icon::new(DatalithIcon::ArrowUpAz).size_4()
        };
        let add_button = (!self.workspace.is_read_only()).then(|| {
            Button::new("todo-add-btn")
                .ghost()
                .icon(IconName::Plus)
                .mr_1()
                .on_click(cx.listener(|this, _, window, cx| {
                    this.new_task_input.focus_handle(cx).focus(window, cx);
                }))
        });

        h_flex()
            .h(px(TODO_HEADER_HEIGHT))
            .w_full()
            .items_center()
            .gap_1()
            .px_3()
            .border_b_1()
            .border_color(self.theme(cx).border)
            .children(add_button)
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
                            } else if !this.workspace.is_read_only() {
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
                            .text_color(self.theme(cx).muted_foreground),
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

    fn render_select(&self, kind: PreviewSelectKind, cx: &Context<Self>) -> AnyElement {
        let state = match kind {
            PreviewSelectKind::Filter => &self.filter_select,
            PreviewSelectKind::Sort => &self.sort_select,
        };
        self.appearance.as_ref().map_or_else(
            || {
                Select::new(state)
                    .bg(self.theme(cx).background)
                    .text_color(self.theme(cx).foreground)
                    .into_any_element()
            },
            |appearance| {
                PreviewSelect {
                    kind,
                    selected_index: state
                        .read(cx)
                        .selected_index(cx)
                        .map_or(0, |index| index.row),
                    editor: cx.entity(),
                    appearance: appearance.clone(),
                }
                .into_any_element()
            },
        )
    }

    fn choose_filter(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.filter_select.update(cx, |state, cx| {
            state.set_selected_index(Some(IndexPath::new(index)), window, cx);
        });
        self.workspace.set_filter(FilterKind::from_index(index));
        self.refresh_item_sizes();
        cx.notify();
    }

    fn choose_sort(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.sort_select.update(cx, |state, cx| {
            state.set_selected_index(Some(IndexPath::new(index)), window, cx);
        });
        self.workspace.set_sort(SortKind::from_index(index));
        self.refresh_item_sizes();
        cx.notify();
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
                .bg(self.theme(cx).warning)
                .text_color(self.theme(cx).warning_foreground)
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
                .text_color(self.theme(cx).muted_foreground)
                .child(
                    Icon::new(IconName::Inbox)
                        .size_8()
                        .text_color(self.theme(cx).muted_foreground.opacity(0.4)),
                )
                .child(div().text_sm().child("No tasks to display"))
                .into_any_element();
        }

        let entity = cx.entity();
        let sizes = self.item_sizes.clone();
        let selected = self.workspace.selected();
        let visible_owned = visible.to_vec();

        v_flex()
            .flex_1()
            .min_h_0()
            .relative()
            .overflow_hidden()
            .child(
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
                .track_scroll(&self.scroll_handle),
            )
            .child(Scrollbar::vertical(&self.scroll_handle))
            .into_any_element()
    }

    fn render_new_task_row(&self, cx: &Context<Self>) -> AnyElement {
        if self.workspace.is_read_only() {
            return div().h(px(TODO_NEW_ROW_HEIGHT)).w_full().into_any_element();
        }
        h_flex()
            .h(px(TODO_NEW_ROW_HEIGHT))
            .w_full()
            .items_center()
            .gap_2()
            .px_3()
            .border_t_1()
            .border_color(self.theme(cx).border)
            .child(div().w(px(TODO_INDENT_PX + 16.0)))
            .child(
                div()
                    .flex_1()
                    .id("new-task-input-wrap")
                    .child(
                        Input::new(&self.new_task_input)
                            .bg(self.theme(cx).background)
                            .text_color(self.theme(cx).foreground)
                            .appearance(false),
                    )
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

#[derive(Clone, Copy)]
enum PreviewSelectKind {
    Filter,
    Sort,
}

impl PreviewSelectKind {
    fn id(self, suffix: &str) -> ElementId {
        ElementId::from(format!("todo-preview-{}-{suffix}", self.as_str()))
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Filter => "filter",
            Self::Sort => "sort",
        }
    }

    fn labels(self) -> Vec<&'static str> {
        match self {
            Self::Filter => FilterKind::ALL.iter().map(|kind| kind.label()).collect(),
            Self::Sort => SortKind::ALL.iter().map(|kind| kind.label()).collect(),
        }
    }
}

#[derive(Default)]
struct PreviewPopupState {
    menu: Option<Entity<PopupMenu>>,
    appearance: Option<Rc<gpui_kit::component::Theme>>,
}

#[derive(IntoElement)]
struct PreviewSelect {
    kind: PreviewSelectKind,
    selected_index: usize,
    editor: Entity<TodoTxtState>,
    appearance: Rc<gpui_kit::component::Theme>,
}

impl RenderOnce for PreviewSelect {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let menu_state = window.use_keyed_state(self.kind.id("menu-state"), cx, |_, _| {
            PreviewPopupState::default()
        });
        let labels = self.kind.labels();
        let selected_label = labels.get(self.selected_index).copied().unwrap_or_default();
        let appearance = self.appearance.clone();
        let select_editor = self.editor.clone();
        let kind = self.kind;
        let selected_index = self.selected_index;

        let previous_appearance = menu_state.read(cx).appearance.clone();
        if previous_appearance
            .as_ref()
            .is_some_and(|previous| !Rc::ptr_eq(previous, &appearance))
            && let Some(menu) = menu_state.read(cx).menu.clone()
        {
            menu.update(cx, |_, cx| cx.notify());
        }
        menu_state.update(cx, |state, _| {
            state.appearance = Some(appearance.clone());
        });

        Popover::new(kind.id("popover"))
            .appearance(false)
            .overlay_closable(false)
            .trigger(
                Button::new(kind.id("trigger"))
                    .outline()
                    .label(selected_label)
                    .dropdown_caret(true)
                    .w_full()
                    .accessibility_label(match kind {
                        PreviewSelectKind::Filter => format!("Filter: {selected_label}"),
                        PreviewSelectKind::Sort => format!("Sort: {selected_label}"),
                    }),
            )
            .content(move |_, window, cx| {
                let existing_menu = menu_state.read(cx).menu.clone();
                let menu = existing_menu.unwrap_or_else(|| {
                    let menu_labels = labels.clone();
                    let menu_editor = select_editor.clone();
                    let menu = PopupMenu::build(window, cx, move |mut menu, _, _| {
                        for (index, label) in menu_labels.into_iter().enumerate() {
                            let item_editor = menu_editor.clone();
                            menu = menu.item(
                                PopupMenuItem::new(label)
                                    .checked(index == selected_index)
                                    .on_click(move |_, window, app| {
                                        app.update_entity(&item_editor, |todo, cx| match kind {
                                            PreviewSelectKind::Filter => {
                                                todo.choose_filter(index, window, cx);
                                            }
                                            PreviewSelectKind::Sort => {
                                                todo.choose_sort(index, window, cx);
                                            }
                                        });
                                    }),
                            );
                        }
                        menu
                    });
                    menu.focus_handle(cx).focus(window, cx);

                    let popover_state = cx.entity();
                    window
                        .subscribe(&menu, cx, {
                            let menu_state = menu_state.clone();
                            move |_, _: &DismissEvent, window, cx| {
                                popover_state.update(cx, |state, cx| {
                                    state.dismiss(window, cx);
                                });
                                menu_state.update(cx, |state, _| state.menu = None);
                            }
                        })
                        .detach();

                    menu_state.update(cx, |state, _| state.menu = Some(menu.clone()));
                    menu
                });

                crate::ui::themes::preview::themed(
                    appearance.clone(),
                    div().id(kind.id("popup")).child(menu),
                )
            })
    }
}
