use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div};
use gpui_component::{ActiveTheme, Icon, v_flex};

use crate::ui::icons::DatalithIcon;

use super::interaction::GraphViewState;
use super::snapshot::ViewerStatus;

impl Render for GraphViewState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.status {
            ViewerStatus::Loading => self.render_centered("Loading Graph View…", cx),
            ViewerStatus::Empty => v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_3()
                .child(
                    Icon::new(DatalithIcon::Graph)
                        .size_8()
                        .text_color(cx.theme().muted_foreground.opacity(0.4)),
                )
                .child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .child("No Markdown files match this Graph Definition."),
                )
                .into_any_element(),
            ViewerStatus::Error(error) => self.render_centered(error.clone(), cx),
            ViewerStatus::Ready(_) => self.render_canvas(window, cx),
        }
    }
}
