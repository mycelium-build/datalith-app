use gpui::{Context, IntoElement, Render, Window};

use super::interaction::GraphViewState;
use super::snapshot::ViewerStatus;

impl Render for GraphViewState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.status {
            ViewerStatus::Loading => self.render_centered("Loading Graph View…", cx),
            ViewerStatus::Empty => {
                self.render_centered("No Markdown files match this Graph Definition.", cx)
            }
            ViewerStatus::Error(error) => self.render_centered(error.clone(), cx),
            ViewerStatus::Ready(_) => self.render_canvas(window, cx),
        }
    }
}
