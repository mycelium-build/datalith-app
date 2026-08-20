pub mod base;
pub mod graph;
pub mod image;
pub mod markdown;

use gpui::{AnyElement, App, Context, Entity, FocusHandle};

use self::base::BaseViewer;
use self::graph::GraphViewer;
use self::image::ImageViewer;
use self::markdown::MarkdownViewer;

use crate::document::handler::FileHandler;
use crate::vault::VaultCatalog;

pub enum ViewerKind {
    Base(BaseViewer),
    Graph(GraphViewer),
    Markdown(MarkdownViewer),
    Image(ImageViewer),
}

impl ViewerKind {
    pub fn render(&self, handler: Entity<FileHandler>, cx: &mut App) -> AnyElement {
        match self {
            Self::Base(viewer) => viewer.render(handler, cx),
            Self::Graph(viewer) => viewer.render(handler, cx),
            Self::Markdown(viewer) => viewer.render(handler, cx),
            Self::Image(viewer) => viewer.render(cx),
        }
    }

    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        match self {
            Self::Base(viewer) => viewer.focus_handle(cx),
            Self::Graph(viewer) => viewer.focus_handle(cx),
            Self::Markdown(viewer) => viewer.focus_handle(cx),
            Self::Image(viewer) => viewer.focus_handle(cx),
        }
    }

    pub fn refresh(&self, cx: &mut App) {
        match self {
            Self::Base(viewer) => viewer.refresh(cx),
            Self::Graph(viewer) => viewer.refresh(cx),
            Self::Markdown(_) | Self::Image(_) => {}
        }
    }

    pub fn set_vault_catalog(&self, catalog: VaultCatalog, cx: &mut Context<FileHandler>) {
        match self {
            Self::Base(viewer) => viewer.set_vault_catalog(catalog, cx),
            Self::Graph(viewer) => viewer.set_vault_catalog(catalog, cx),
            Self::Markdown(_) | Self::Image(_) => {}
        }
    }
}
