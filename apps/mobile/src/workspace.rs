use std::path::{Path, PathBuf};

use datalith_core::file_types::registered_file_types;
use datalith_ui::handler::{FileHandler, FileHandlerEvent, ViewMode};
use datalith_ui::registry::{FileRegistry, ViewerDependencies, default_registry};
use datalith_vault::{VaultCatalog, file_ops};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::InputEvent;
use gpui_kit::component::{ActiveTheme, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    BackGesturePhase, Context, Entity, Focusable, Render, Subscription, Window, div, px,
};

struct Document {
    path: PathBuf,
    handler: Entity<FileHandler>,
    _subscriptions: Vec<Subscription>,
}

pub struct Workspace {
    pub(super) root: Option<PathBuf>,
    folders: Vec<super::browser::Folder>,
    focus: gpui_kit::FocusHandle,
    catalog: Option<VaultCatalog>,
    registry: FileRegistry,
    history: Vec<Document>,
    navigation_step: usize,
    returning: bool,
    deleted: Option<file_ops::Deleted>,
    message: Option<String>,
    document_deleted: bool,
}

impl Workspace {
    pub(super) fn new(
        root: anyhow::Result<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe_window_appearance(window, |_, window, cx| {
            gpui_kit::component::Theme::sync_system_appearance(Some(window), cx);
            cx.notify();
        })
        .detach();
        let mut view = Self {
            root: None,
            folders: Vec::new(),
            focus: cx.focus_handle(),
            catalog: None,
            registry: default_registry(),
            history: Vec::new(),
            navigation_step: 0,
            returning: false,
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
                view.select(root, window, cx);
                view.focus.focus(window, cx);
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
        self.advance(false);
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

    pub(super) fn select(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        window.dismiss_virtual_keyboard();
        self.focus.focus(window, cx);
        if path.is_dir() {
            match super::browser::Folder::new(path, window, cx) {
                Ok(folder) => {
                    if !self.folders.is_empty() {
                        self.advance(false);
                    }
                    self.folders.push(folder);
                    self.history.clear();
                    self.document_deleted = false;
                }
                Err(error) => self.message = Some(error.to_string()),
            }
        } else if self.in_vault(&path) {
            self.history.clear();
            self.open(path, window, cx);
        }
        cx.notify();
    }

    const fn advance(&mut self, returning: bool) {
        self.navigation_step = self.navigation_step.wrapping_add(1);
        self.returning = returning;
    }

    fn refresh_folder(&mut self) {
        if let Some(folder) = self.folders.last_mut()
            && let Err(error) = folder.reload()
        {
            self.message = Some(error.to_string());
        }
    }

    pub(super) fn back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.dismiss_virtual_keyboard();
        self.focus.focus(window, cx);
        self.advance(true);
        if self.document_deleted {
            self.document_deleted = false;
        } else if self.history.pop().is_none() && self.folders.len() > 1 {
            self.folders.pop();
            self.refresh_folder();
        }
        cx.notify();
    }

    pub(super) fn toggle_mode(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(doc) = self.current() {
            doc.handler.update(cx, |handler, cx| {
                if handler.can_toggle_mode() {
                    let mode = if handler.is_editing() {
                        ViewMode::View
                    } else {
                        ViewMode::Edit
                    };
                    handler.set_mode(mode, cx);
                    handler.focus_handle(cx).focus(window, cx);
                    if mode == ViewMode::Edit && handler.input().is_some() {
                        window.request_virtual_keyboard();
                    } else {
                        window.dismiss_virtual_keyboard();
                    }
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
                self.history.retain(|doc| doc.path != deleted.original);
                self.document_deleted = true;
                self.deleted = Some(deleted);
                self.refresh_folder();
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
                self.refresh_folder();
                if self.document_deleted {
                    self.open(path, window, cx);
                }
            }
            Err(error) => self.message = Some(error.to_string()),
        }
        cx.notify();
    }
}

pub fn name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

impl Workspace {
    fn navigation_content(
        &self,
        content: gpui_kit::Div,
        wide: bool,
        cx: &Context<Self>,
    ) -> gpui_kit::AnyElement {
        let reduce_motion = cx.reduce_motion();
        #[cfg(target_os = "ios")]
        let reduce_motion = reduce_motion || super::ios::reduce_motion();
        if !wide && self.navigation_step > 0 && !reduce_motion {
            use gpui_kit::{Animation, AnimationExt as _};
            let offset = if self.returning { -24.0 } else { 24.0 };
            let transition = Animation::new(std::time::Duration::from_millis(190))
                .with_easing(gpui_kit::ease_out_quint());
            content
                .relative()
                .with_animation(
                    ("navigation-step", self.navigation_step),
                    transition,
                    move |el, progress| el.left(px((1.0 - progress) * offset)).opacity(progress),
                )
                .into_any_element()
        } else {
            content.into_any_element()
        }
    }

    fn detail(&self, wide: bool, has_detail: bool, cx: &Context<Self>) -> impl IntoElement {
        let mut detail = v_flex().flex_1().min_w_0();
        let mut bar = h_flex()
            .gap_1()
            .px_2()
            .min_h(px(48.))
            .border_b_1()
            .border_color(cx.theme().border);
        if has_detail && (!wide || self.history.len() > 1) {
            bar = bar.child(
                Button::new("back")
                    .ghost()
                    .h(px(48.))
                    .icon(gpui_kit::component::IconName::ChevronLeft)
                    .w(px(48.))
                    .accessibility_label("Back")
                    .on_click(cx.listener(|v, _, w, cx| v.back(w, cx))),
            );
        }
        if let Some(doc) = self.current() {
            let h = doc.handler.read(cx);
            bar = bar.child(div().flex_1().truncate().child(name(&doc.path)));
            if h.can_toggle_mode() {
                bar = bar.child(
                    Button::new("mode")
                        .ghost()
                        .icon(gpui_kit::component::Icon::new(if h.is_editing() {
                            datalith_ui::icons::DatalithIcon::Book
                        } else {
                            datalith_ui::icons::DatalithIcon::Pen
                        }))
                        .w(px(48.))
                        .accessibility_label(if h.is_editing() { "Read" } else { "Edit" })
                        .h(px(48.))
                        .on_click(cx.listener(|v, _, w, cx| v.toggle_mode(w, cx))),
                );
            }
            if !h.is_read_only() {
                bar = bar.child(
                    Button::new("delete")
                        .ghost()
                        .icon(gpui_kit::component::IconName::Delete)
                        .w(px(48.))
                        .accessibility_label("Delete")
                        .h(px(48.))
                        .on_click(cx.listener(|v, _, _, cx| v.delete(cx))),
                );
            }
            let body = div().flex_1().min_h_0().child(doc.handler.clone());
            #[cfg(target_os = "ios")]
            let body = body.when(h.is_editing() && !h.is_read_only(), |el| {
                el.children(h.input().map(super::ios_input::registration))
            });
            detail = detail
                .when(!cfg!(target_os = "ios"), |el| el.child(bar))
                .child(body);
        } else {
            detail = detail
                .when(has_detail && !cfg!(target_os = "ios"), |el| el.child(bar))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(cx.theme().muted_foreground)
                        .child(if self.document_deleted {
                            "File deleted"
                        } else {
                            "Select a file"
                        }),
                );
        }
        detail
    }
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
        let wide = bounds.size.width >= px(700.);
        let has_detail = self.current().is_some() || self.document_deleted;
        let can_back = has_detail || self.folders.len() > 1;
        #[cfg(target_os = "ios")]
        super::ios::update(
            can_back,
            self.current().map(|d| d.handler.read(cx)),
            cx.theme().background,
        );
        let show_browser = wide || !has_detail;
        let mut content = h_flex().flex_1().min_h_0().items_stretch();
        if show_browser && let Some(folder) = self.folders.last() {
            let mut browser = v_flex()
                .min_w_0()
                .when(wide, |el| {
                    el.w(px(272.)).border_r_1().border_color(cx.theme().border)
                })
                .when(!wide, gpui_kit::Styled::flex_1);
            if self.folders.len() > 1 && (!cfg!(target_os = "ios") || (wide && has_detail)) {
                browser = browser.child(
                    h_flex().px_2().child(
                        Button::new("parent")
                            .ghost()
                            .h(px(48.))
                            .icon(gpui_kit::component::IconName::ChevronLeft)
                            .w(px(48.))
                            .accessibility_label("Back")
                            .on_click(cx.listener(|v, _, w, cx| {
                                v.history.clear();
                                v.document_deleted = false;
                                v.back(w, cx);
                            })),
                    ),
                );
            }
            content = content.child(browser.child(folder.render(
                &self.registry,
                wide || !cfg!(target_os = "ios"),
                self.current().map(|d| d.path.as_path()),
                cx,
            )));
        }
        if wide || has_detail {
            content = content.child(self.detail(wide, has_detail, cx));
        }
        panel = panel
            .overflow_hidden()
            .child(self.navigation_content(content, wide, cx));
        panel = panel.track_focus(&self.focus);
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
        panel.when(can_back, |el| {
            el.on_key_down(cx.listener(|v, event: &gpui_kit::KeyDownEvent, w, cx| {
                if event.keystroke.key == "escape" {
                    v.back(w, cx);
                }
            }))
            .on_back_gesture(cx.listener(
                |v, event: &gpui_kit::BackGestureEvent, w, cx| {
                    if event.phase == BackGesturePhase::Committed {
                        v.back(w, cx);
                    }
                },
            ))
        })
    }
}
