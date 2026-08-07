pub mod graph;
pub mod image;
pub mod markdown;

use gpui::{AnyElement, App, Context, Entity, FocusHandle, Window};

use self::graph::GraphViewer;
use self::image::ImageViewer;
use self::markdown::MarkdownViewer;

use crate::document::handler::FileHandler;
use crate::vault::VaultCatalog;

pub enum ViewerKind {
    Graph(GraphViewer),
    Markdown(MarkdownViewer),
    Image(ImageViewer),
}

impl ViewerKind {
    pub fn render(
        &self,
        handler: Entity<FileHandler>,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        match self {
            Self::Graph(viewer) => viewer.render(handler, cx),
            Self::Markdown(viewer) => viewer.render(handler, window, cx),
            Self::Image(viewer) => viewer.render(cx),
        }
    }

    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        match self {
            Self::Graph(viewer) => viewer.focus_handle(cx),
            Self::Markdown(viewer) => viewer.focus_handle(cx),
            Self::Image(viewer) => viewer.focus_handle(cx),
        }
    }

    pub fn refresh(&self, cx: &mut App) {
        if let Self::Graph(viewer) = self {
            viewer.refresh(cx);
        }
    }

    pub fn set_vault_catalog(&self, catalog: VaultCatalog, cx: &mut Context<FileHandler>) {
        if let Self::Graph(viewer) = self {
            viewer.set_vault_catalog(catalog, cx);
        }
    }
}
