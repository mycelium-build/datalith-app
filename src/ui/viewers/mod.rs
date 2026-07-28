pub(crate) mod graph;
pub(crate) mod image;
pub(crate) mod markdown;

use gpui::*;

use self::graph::GraphViewer;
use self::image::ImageViewer;
use self::markdown::MarkdownViewer;

use crate::document::handler::FileHandler;
use crate::vault::VaultCatalog;

pub(crate) enum ViewerKind {
    Graph(GraphViewer),
    Markdown(MarkdownViewer),
    Image(ImageViewer),
}

impl ViewerKind {
    pub(crate) fn render(&self, handler: Entity<FileHandler>, cx: &mut App) -> AnyElement {
        match self {
            ViewerKind::Graph(viewer) => viewer.render(handler, cx),
            ViewerKind::Markdown(viewer) => viewer.render(handler, cx),
            ViewerKind::Image(viewer) => viewer.render(cx),
        }
    }

    pub(crate) fn focus_handle(&self, cx: &App) -> FocusHandle {
        match self {
            ViewerKind::Graph(viewer) => viewer.focus_handle(cx),
            ViewerKind::Markdown(viewer) => viewer.focus_handle(cx),
            ViewerKind::Image(viewer) => viewer.focus_handle(cx),
        }
    }

    pub(crate) fn refresh(&self, cx: &mut App) {
        if let ViewerKind::Graph(viewer) = self {
            viewer.refresh(cx);
        }
    }

    pub(crate) fn set_vault_catalog(&self, catalog: VaultCatalog, cx: &mut Context<FileHandler>) {
        if let ViewerKind::Graph(viewer) = self {
            viewer.set_vault_catalog(catalog, cx);
        }
    }
}
