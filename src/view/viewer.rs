use std::cell::Cell;
use std::rc::Rc;

use gpui::*;

use super::viewers::image::ImageViewer;
use super::viewers::markdown::MarkdownViewer;

pub(crate) type SharedEvents = Rc<Cell<Vec<MarkdownViewerEvent>>>;

#[derive(Clone, Debug)]
pub(crate) enum MarkdownViewerEvent {
    LinkClicked(String, bool),
}

pub(crate) enum ViewerKind {
    Markdown(MarkdownViewer),
    Image(ImageViewer),
}

impl ViewerKind {
    pub(crate) fn render(&mut self, cx: &mut App) -> AnyElement {
        match self {
            ViewerKind::Markdown(viewer) => viewer.render(cx),
            ViewerKind::Image(viewer) => viewer.render(cx),
        }
    }

    pub(crate) fn focus_handle(&self, cx: &App) -> FocusHandle {
        match self {
            ViewerKind::Markdown(viewer) => viewer.focus_handle(cx),
            ViewerKind::Image(viewer) => viewer.focus_handle(cx),
        }
    }

    pub(crate) fn supports_editing(&self) -> bool {
        match self {
            ViewerKind::Markdown(_) => true,
            ViewerKind::Image(_) => false,
        }
    }

    pub(crate) fn drain_events(&self) -> Vec<MarkdownViewerEvent> {
        match self {
            ViewerKind::Markdown(viewer) => viewer.drain_events(),
            _ => Vec::new(),
        }
    }
}
