use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, ElementId, Entity, FontWeight, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, RenderOnce, Styled, Window, div, px,
};
use gpui_component::{ActiveTheme, Icon, IconName, Selectable};

use txtodo::Priority;

use super::TodoTxtState;
use super::constants::{TODO_COL_CHECK, TODO_COL_PRIORITY, TODO_PILL_RADIUS};

pub(super) const PRIORITY_VALUES: [Option<char>; 6] =
    [None, Some('A'), Some('B'), Some('C'), Some('D'), Some('E')];

#[derive(IntoElement)]
pub(super) struct PriorityTrigger {
    pub(super) id: ElementId,
    pub(super) editor: Entity<TodoTxtState>,
    pub(super) task_index: usize,
    pub(super) current: Option<Priority>,
    pub(super) completed: bool,
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
                super::priority_color(p.as_char())
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
                    let max_idx = PRIORITY_VALUES.len().saturating_sub(1);
                    let new_idx = if key == "up" {
                        current_idx.checked_sub(1).unwrap_or(max_idx)
                    } else {
                        current_idx
                            .checked_add(1)
                            .filter(|&n| n < PRIORITY_VALUES.len())
                            .unwrap_or(0)
                    };
                    let value = PRIORITY_VALUES
                        .get(new_idx)
                        .copied()
                        .unwrap_or_default()
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
