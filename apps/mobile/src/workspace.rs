use std::path::{Path, PathBuf};

use datalith_core::file_types::registered_file_types;
use datalith_ui::handler::{FileHandler, FileHandlerEvent, ViewMode};
use datalith_ui::registry::{FileRegistry, ViewerDependencies, default_registry};
use datalith_vault::{VaultCatalog, file_ops};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::InputEvent;
use gpui_kit::component::{ActiveTheme, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{BackGesturePhase, Context, Entity, Render, Subscription, Window, div, px};

struct Document {
    path: PathBuf,
    handler: Entity<FileHandler>,
    _subscriptions: Vec<Subscription>,
}

pub struct Workspace {
    pub(super) root: Option<PathBuf>,
    catalog: Option<VaultCatalog>,
    registry: FileRegistry,
    history: Vec<Document>,
    deleted: Option<file_ops::Deleted>,
    message: Option<String>,
    document_deleted: bool,
}

impl Workspace {
    pub(super) fn new(
        root: anyhow::Result<PathBuf>,
        selected: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut view = Self {
            root: None,
            catalog: None,
            registry: default_registry(),
            history: Vec::new(),
            deleted: None,
            message: None,
            document_deleted: false,
        };
        match root {
            Ok(root) => {
                match VaultCatalog::open(&root, registered_file_types()) {
                    Ok(catalog) => view.catalog = Some(catalog),
                    Err(error) => view.message = Some(error.to_string()),
                }
                view.root = Some(root.clone());
                let path = selected.unwrap_or_else(|| root.join("Welcome.md"));
                if path.is_file() {
                    view.open(path, window, cx);
                }
            }
            Err(error) => view.message = Some(error.to_string()),
        }
        view
    }

    fn current(&self) -> Option<&Document> {
        self.history.last().filter(|_| !self.document_deleted)
    }

    fn in_vault(&self, path: &Path) -> bool {
        let Some(root) = &self.root else { return false };
        let Ok(relative) = path.strip_prefix(root) else {
            return false;
        };
        relative.components().all(|component|matches!(component,std::path::Component::Normal(name) if name != ".datalith"))
            && path.is_file()
            && path.canonicalize().ok().zip(root.canonicalize().ok()).is_some_and(|(file,root)|file.starts_with(root))
    }

    pub(super) fn open(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if !self.in_vault(&path) {
            return;
        }
        self.document_deleted = false;
        if let Some(index) = self.history.iter().rposition(|doc| doc.path == path) {
            self.history.truncate(index.saturating_add(1));
            cx.notify();
            return;
        }
        let deps = ViewerDependencies::new(self.catalog.clone());
        let handler = cx.new(|cx| self.registry.create_handler(&path, &deps, window, cx));
        handler.update(cx, |h, cx| h.set_mode(ViewMode::View, cx));
        let mut subscriptions = Vec::new();
        if !handler.read(cx).is_read_only()
            && let Some(input) = handler.read(cx).input().cloned()
        {
            let path = path.clone();
            subscriptions.push(cx.subscribe(&input, move |view, input, event, cx| {
                if matches!(event, InputEvent::Change)
                    && let Err(error) = file_ops::update(&path, &input.read(cx).value())
                {
                    view.message = Some(error.to_string());
                    cx.notify();
                }
            }));
        }
        let from = path.clone();
        subscriptions.push(cx.subscribe_in(
            &handler,
            window,
            move |view, _, FileHandlerEvent::LinkClicked(url, _), window, cx| {
                let raw = url.split(['#', '?']).next().unwrap_or_default();
                let target = percent_encoding::percent_decode_str(raw).decode_utf8_lossy();
                let resolved = view
                    .root
                    .as_ref()
                    .and_then(|root| {
                        datalith_vault::path::resolve_relative_link(&from, &target, root)
                    })
                    .and_then(|path| {
                        if view.in_vault(&path) {
                            Some(path)
                        } else if path.extension().is_none()
                            && view.in_vault(&path.with_extension("md"))
                        {
                            Some(path.with_extension("md"))
                        } else {
                            None
                        }
                    })
                    .or_else(|| view.catalog.as_ref()?.resolve(&target));
                if let Some(path) = resolved {
                    #[cfg(target_os = "ios")]
                    {
                        let _ = window;
                        super::ios::show_link(&path);
                    }
                    #[cfg(not(target_os = "ios"))]
                    view.open(path, window, cx);
                } else if !target.is_empty() {
                    view.message = Some(format!("Cannot open {target}"));
                    cx.notify();
                }
            },
        ));
        self.history.push(Document {
            path,
            handler,
            _subscriptions: subscriptions,
        });
        cx.notify();
    }

    fn back(&mut self, cx: &mut Context<Self>) {
        if self.history.len() > 1 {
            self.history.pop();
            self.document_deleted = false;
        }
        cx.notify();
    }

    pub(super) fn toggle_mode(&self, cx: &mut Context<Self>) {
        if let Some(doc) = self.current() {
            doc.handler.update(cx, |handler, cx| {
                if handler.can_toggle_mode() {
                    let mode = if handler.is_editing() {
                        ViewMode::View
                    } else {
                        ViewMode::Edit
                    };
                    handler.set_mode(mode, cx);
                }
            });
            cx.notify();
        }
    }

    pub(super) fn delete(&mut self, cx: &mut Context<Self>) {
        let (Some(root), Some(doc)) = (&self.root, self.current()) else {
            return;
        };
        match file_ops::delete(root, &doc.path) {
            Ok(deleted) => {
                self.document_deleted = true;
                self.deleted = Some(deleted);
                #[cfg(target_os = "ios")]
                super::ios::refresh_files();
            }
            Err(error) => self.message = Some(error.to_string()),
        }
        cx.notify();
    }

    fn undo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(deleted) = &self.deleted else { return };
        match file_ops::restore(deleted) {
            Ok(()) => {
                let path = deleted.original.clone();
                self.deleted = None;
                #[cfg(target_os = "ios")]
                super::ios::refresh_files();
                if self.document_deleted {
                    self.open(path, window, cx);
                }
            }
            Err(error) => self.message = Some(error.to_string()),
        }
        cx.notify();
    }
}

fn name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bounds = window.fully_visible_bounds();
        let mut panel = v_flex()
            .absolute()
            .top(bounds.origin.y)
            .left(bounds.origin.x)
            .w(bounds.size.width)
            .h(bounds.size.height)
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground);
        #[cfg(target_os = "ios")]
        super::ios::update_controls(self.current().map(|doc| doc.handler.read(cx)));
        {
            let mut bar = h_flex().gap_1().px_2().min_h(px(48.));
            if self.history.len() > 1 {
                bar = bar.child(
                    Button::new("back")
                        .ghost()
                        .h(px(44.))
                        .label("Back")
                        .on_click(cx.listener(|v, _, _, cx| v.back(cx))),
                );
            }
            if let Some(doc) = self.current() {
                let handler = doc.handler.clone();
                if handler.read(cx).can_toggle_mode() {
                    bar = bar.child(
                        Button::new("mode")
                            .ghost()
                            .label(if handler.read(cx).is_editing() {
                                "Read"
                            } else {
                                "Edit"
                            })
                            .h(px(44.))
                            .on_click(cx.listener(|v, _, _, cx| v.toggle_mode(cx))),
                    );
                }
                if !doc.handler.read(cx).is_read_only() {
                    bar = bar.child(
                        Button::new("delete")
                            .ghost()
                            .label("Delete")
                            .h(px(44.))
                            .on_click(cx.listener(|v, _, _, cx| v.delete(cx))),
                    );
                }
                panel = panel
                    .when(!cfg!(target_os = "ios"), |el| el.child(bar))
                    .child(div().flex_1().min_h_0().child(doc.handler.clone()));
            } else {
                panel = panel
                    .when(!cfg!(target_os = "ios"), |el| el.child(bar))
                    .child(div().flex_1().child(if self.document_deleted {
                        "File deleted"
                    } else {
                        "Select a file"
                    }));
            }
        }
        if let Some(error) = &self.message {
            panel = panel.child(div().px_2().child(error.clone()));
        }
        if let Some(deleted) = &self.deleted {
            panel = panel.child(
                h_flex()
                    .px_3()
                    .min_h(px(52.))
                    .justify_between()
                    .bg(cx.theme().secondary)
                    .child(format!("Deleted {}", name(&deleted.original)))
                    .child(
                        Button::new("undo")
                            .label("Undo")
                            .h(px(44.))
                            .on_click(cx.listener(|v, _, w, cx| v.undo(w, cx))),
                    ),
            );
        }
        panel.when(!cfg!(target_os = "ios") && self.history.len() > 1, |el| {
            el.on_back_gesture(cx.listener(|v, event: &gpui_kit::BackGestureEvent, _, cx| {
                if event.phase == BackGesturePhase::Committed {
                    v.back(cx);
                }
            }))
        })
    }
}
