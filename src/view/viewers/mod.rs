pub(crate) mod image;
pub(crate) mod markdown;

use gpui::*;

use self::image::ImageViewer;
use self::markdown::MarkdownViewer;

use super::file_handler::FileHandler;

pub(crate) enum ViewerKind {
    Markdown(MarkdownViewer),
    Image(ImageViewer),
}

impl ViewerKind {
    pub(crate) fn render(&self, handler: Entity<FileHandler>, cx: &mut App) -> AnyElement {
        match self {
            ViewerKind::Markdown(viewer) => viewer.render(handler, cx),
            ViewerKind::Image(viewer) => viewer.render(cx),
        }
    }

    pub(crate) fn focus_handle(&self, cx: &App) -> FocusHandle {
        match self {
            ViewerKind::Markdown(viewer) => viewer.focus_handle(cx),
            ViewerKind::Image(viewer) => viewer.focus_handle(cx),
        }
    }
}
