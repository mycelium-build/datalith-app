use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
mod panels;
use datalith_vault::source;
use panels::{Page, Preferences};

mod files;
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
    edit: Option<crate::draft::Edit>,
    saving: bool,
    _subscriptions: Vec<Subscription>,
}

pub struct Workspace {
    pub(super) root: Option<PathBuf>,
    folders: Vec<super::browser::Folder>,
    focus: gpui_kit::FocusHandle,
    pub(super) catalog: Option<VaultCatalog>,
    pub(super) registry: FileRegistry,
    documents: BTreeMap<PathBuf, Document>,
    history: Vec<PathBuf>,
    page: Option<Page>,
    preferences: Preferences,
    notice_index: usize,
    file_form: Option<files::FileForm>,
    local_root: Option<PathBuf>,
    #[cfg(any(target_os = "ios", target_os = "android"))]
    external: Option<crate::external_folder::Folder>,
    search_task: Option<gpui_kit::Task<()>>,
    pub(super) search_results: Vec<PathBuf>,

    pub(super) whole_vault: bool,
    navigation_step: usize,
    returning: bool,
    deleted: Option<file_ops::Deleted>,
    message: Option<String>,
    deleted_document: Option<PathBuf>,
}

impl Workspace {
    pub(super) fn new(
        root: anyhow::Result<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe_window_appearance(window, |state: &mut Self, window, cx| {
            gpui_kit::component::Theme::sync_system_appearance(Some(window), cx);
            state.preferences.apply_size(cx);
            cx.notify();
        })
        .detach();
        cx.observe_window_activation(window, |state: &mut Self, w, cx| {
            if w.is_window_active() && state.root.as_deref().is_some_and(source::is_provider) {
                state.refresh_folder();
                if let Some(p) = state.history.last().cloned() {
                    state.refresh_document(&p, w, cx);
                }
                if let Some(c) = &state.catalog {
                    let catalog = c.clone();
                    cx.background_spawn(async move {
                        catalog.refresh_paths(vec![catalog.root()]);
                    })
                    .detach();
                }
                cx.notify();
            }
        })
        .detach();
        let mut view = Self {
            root: None,
            folders: Vec::new(),
            focus: cx.focus_handle(),
            catalog: None,
            registry: default_registry(),
            history: Vec::new(),
            documents: BTreeMap::new(),
            page: None,
            notice_index: 0,
            file_form: None,
            preferences: Preferences::load(),
            local_root: None,
            #[cfg(any(target_os = "ios", target_os = "android"))]
            external: None,
            search_task: None,
            search_results: Vec::new(),

            whole_vault: false,
            navigation_step: 0,
            returning: false,
            deleted: None,
            message: None,
            deleted_document: None,
        };
        match root {
            Ok(root) => {
                view.local_root = Some(root.clone());
                view.preferences.apply_theme(window, cx);
                let active = if view.preferences.documentation {
                    super::DOCS.root()
                } else {
                    root
                };
                #[cfg(any(target_os = "ios", target_os = "android"))]
                let restored = view.restore_external(window, cx);
                #[cfg(not(any(target_os = "ios", target_os = "android")))]
                let restored: anyhow::Result<bool> = Ok(false);
                if !matches!(restored, Ok(true)) {
                    view.switch_vault(active, true, window, cx);
                    if let Err(err) = restored {
                        view.message = Some(err.to_string());
                    }
                }
                view.focus.focus(window, cx);
            }
            Err(error) => view.message = Some(error.to_string()),
        }
        if view.folders.is_empty() {
            view.page = Some(Page::Library);
        }
        view
    }

    pub(super) fn folder_name(&self, path: &Path) -> String {
        #[cfg(any(target_os = "android", target_os = "ios"))]
        if let Some(folder) = &self.external
            && folder.path == path
        {
            return folder.label.clone();
        }
        if self.root.as_deref() == Some(path)
            && let Some(vault) = source::embedded_vault(path)
        {
            return vault.name().into();
        }
        name(path)
    }
    fn current(&self) -> Option<&Document> {
        self.history
            .last()
            .and_then(|path| self.documents.get(path))
            .filter(|_| self.deleted_document.is_none())
    }

    fn in_vault(&self, path: &Path) -> bool {
        let Some(root) = &self.root else { return false };
        let Ok(relative) = path.strip_prefix(root) else {
            return false;
        };
        if source::is_read_only(root) || source::is_provider(root) {
            return source::is_file(path);
        }
        relative.components().all(|component|matches!(component,std::path::Component::Normal(name) if name != ".datalith"))
            && path.is_file()
            && path.canonicalize().ok().zip(root.canonicalize().ok()).is_some_and(|(file,root)|file.starts_with(root))
    }

    pub(super) fn open(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if !self.in_vault(&path) {
            return;
        }
        self.message = None;
        self.advance(false);
        self.deleted_document = None;
        self.page = None;
        if self.history.last() != Some(&path) {
            self.history.push(path.clone());
        }
        if self.documents.contains_key(&path) {
            self.refresh_document(&path, window, cx);
            cx.notify();
            return;
        }
        let deps = ViewerDependencies::new(self.catalog.clone());
        let handler = cx.new(|cx| self.registry.create_handler(&path, &deps, window, cx));
        handler.update(cx, |h, cx| h.set_mode(ViewMode::View, cx));
        let mut subscriptions = Vec::new();
        let mut edit = None;
        if !handler.read(cx).is_read_only()
            && let Some(input) = handler.read(cx).input().cloned()
        {
            let path = path.clone();
            if source::is_provider(&path) {
                match crate::draft::Edit::open(&path, input.read(cx).value().to_string()) {
                    Ok(state) => {
                        if let Some(pending) = &state.pending {
                            input.update(cx, |i, cx| i.set_value(pending, window, cx));
                            self.message = Some("Recovered local edits awaiting save".into());
                        }
                        edit = Some(state);
                    }
                    Err(e) => {
                        self.message = Some(e.to_string());
                        cx.notify();
                        return;
                    }
                }
            }
            subscriptions.push(cx.subscribe(&input, move |view, input, event, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                let text = input.read(cx).value();
                let result = if source::is_provider(&path) {
                    let Some(edit) = view.documents.get_mut(&path).and_then(|d| d.edit.as_mut())
                    else {
                        return;
                    };
                    edit.stage(&text)
                } else {
                    file_ops::update(&path, &text)
                };
                if let Err(e) = result {
                    view.message = Some(format!("Unsaved edits in {}. {e}", name(&path)));
                    cx.notify();
                } else if source::is_provider(&path) {
                    view.schedule_save(&path, cx);
                }
            }));
        }
        subscriptions.push(Self::observe_links(path.clone(), &handler, window, cx));
        self.documents.insert(
            path.clone(),
            Document {
                path,
                handler,
                edit,
                saving: false,
                _subscriptions: subscriptions,
            },
        );
        cx.notify();
    }

    fn schedule_save(&mut self, path: &Path, cx: &Context<Self>) {
        let Some(doc) = self.documents.get_mut(path) else {
            return;
        };
        if doc.saving || doc.edit.as_ref().is_none_or(|e| e.pending.is_none()) {
            return;
        }
        doc.saving = true;
        let path = path.to_owned();
        let handler = doc.handler.entity_id();
        let catalog = self.catalog.clone();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(350))
                .await;
            let request = this
                .update(cx, |s, _| {
                    s.documents
                        .get(&path)
                        .filter(|d| d.handler.entity_id() == handler)?
                        .edit
                        .as_ref()?
                        .write_request()
                })
                .ok()
                .flatten();
            let Some(request) = request else {
                return;
            };
            let changed = path.clone();
            let result = cx
                .background_spawn(async move {
                    let saved = request.commit()?;
                    if let Some(c) = catalog {
                        c.refresh_paths(vec![changed]);
                    }
                    anyhow::Ok(saved)
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                let Some(doc) = s
                    .documents
                    .get_mut(&path)
                    .filter(|d| d.handler.entity_id() == handler)
                else {
                    return;
                };
                doc.saving = false;
                let Some(edit) = &mut doc.edit else {
                    return;
                };
                match result.and_then(|saved| edit.accept(saved)) {
                    Ok(()) => s.schedule_save(&path, cx),
                    Err(e) => s.message = Some(format!("Unsaved edits in {}. {e}", name(&path))),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn observe_links(
        from: PathBuf,
        handler: &Entity<FileHandler>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe_in(handler, window, move |view, _, event, window, cx| {
            let url = match event {
                FileHandlerEvent::VaultFileRequested(relative, _) => {
                    if let Some(path) = view.root.as_ref().map(|root| root.join(relative)) {
                        if view.in_vault(&path) {
                            view.open(path, window, cx);
                        } else {
                            view.message = Some(format!("Cannot open {}", relative.display()));
                            cx.notify();
                        }
                    }
                    return;
                }
                FileHandlerEvent::LinkClicked(url, _) => url,
            };
            let raw = url.split(['#', '?']).next().unwrap_or_default();
            let target = percent_encoding::percent_decode_str(raw).decode_utf8_lossy();
            let resolved = view
                .root
                .as_ref()
                .and_then(|root| datalith_vault::path::resolve_relative_link(&from, &target, root))
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
        })
    }
    fn refresh_document(&mut self, path: &Path, w: &mut Window, cx: &mut Context<Self>) {
        let Some(doc) = self.documents.get_mut(path) else {
            return;
        };
        if let Some(edit) = &mut doc.edit {
            match edit.refresh() {
                Ok(Some(text)) => {
                    if let Some(input) = doc.handler.read(cx).input().cloned() {
                        input.update(cx, |i, cx| i.set_value(text, w, cx));
                    }
                }
                Ok(None) => {}
                Err(e) => self.message = Some(e.to_string()),
            }
        } else if let Err(e) = doc
            .handler
            .update(cx, |h, cx| h.reload_from_disk(path, w, cx))
        {
            self.message = Some(e.to_string());
        }
    }
    fn resolve_draft(&mut self, copy: bool, w: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.history.last().cloned() else {
            return;
        };
        let Some(edit) = self.documents.get_mut(&path).and_then(|d| d.edit.as_mut()) else {
            return;
        };
        let result = if copy {
            edit.keep_both().map(Some)
        } else if let Some(text) = edit.pending.clone() {
            edit.save(&text).map(|()| None)
        } else {
            return;
        };
        match result {
            Ok(new) => {
                self.message = None;
                if let Some(new) = new {
                    self.documents.remove(&path);
                    if let Some(c) = &self.catalog {
                        c.refresh_paths(vec![new.clone()]);
                    }
                    self.open(new, w, cx);
                }
            }
            Err(e) => self.message = Some(e.to_string()),
        }
        cx.notify();
    }

    pub(super) fn select(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        window.dismiss_virtual_keyboard();
        self.focus.focus(window, cx);
        if source::is_dir(&path) {
            match super::browser::Folder::new(path, window, cx) {
                Ok(folder) => {
                    if !self.folders.is_empty() {
                        self.advance(false);
                    }
                    self.folders.push(folder);
                    self.search_task = None;
                    self.search_results.clear();

                    self.remember_folder(cx);
                    self.history.clear();
                    self.deleted_document = None;
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
        self.message = None;
        if self.file_form.take().is_some() {
            cx.notify();
            return;
        }
        if let Some(page) = self.page.take() {
            self.page = page.parent();
            cx.notify();
            return;
        }
        if let Some(doc) = self.current()
            && doc.handler.update(cx, |h, cx| h.dismiss_view_control(cx))
        {
            return;
        }

        if self.deleted_document.is_some() {
            self.deleted_document = None;
        } else if self.history.pop().is_none() && self.folders.len() > 1 {
            self.folders.pop();
            self.refresh_folder();
            self.remember_folder(cx);
            self.search_changed(cx);
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
        if let Some(path) = self.current().map(|doc| doc.path.clone()) {
            self.remove_file(&path, cx);
        }
    }

    fn undo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(deleted) = &self.deleted else { return };
        match file_ops::restore(deleted) {
            Ok(()) => {
                let path = deleted.original.clone();
                self.deleted = None;
                self.changed_files(vec![path], cx);
                if let Some(document) = self.deleted_document.take() {
                    self.open(document, window, cx);
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
    fn feedback(&self, mut panel: gpui_kit::Div, cx: &Context<Self>) -> gpui_kit::Div {
        if let Some(error) = &self.message {
            panel = panel.child(
                h_flex()
                    .px_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .whitespace_normal()
                            .child(error.clone()),
                    )
                    .child(
                        Button::new("dismiss-error")
                            .ghost()
                            .icon(gpui_kit::component::IconName::Close)
                            .accessibility_label("Dismiss error")
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.message = None;
                                cx.notify();
                            })),
                    ),
            );
        }
        if self
            .current()
            .filter(|doc| !doc.saving)
            .and_then(|doc| doc.edit.as_ref())
            .is_some_and(|edit| edit.pending.is_some())
        {
            panel = panel.child(
                h_flex()
                    .child(
                        Button::new("retry-write")
                            .label("Retry save")
                            .on_click(cx.listener(|s, _, w, cx| s.resolve_draft(false, w, cx))),
                    )
                    .child(
                        Button::new("keep-both")
                            .label("Keep both")
                            .on_click(cx.listener(|s, _, w, cx| s.resolve_draft(true, w, cx))),
                    ),
            );
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
        panel
    }
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
            bar = bar
                .child(div().flex_1().truncate().child(name(&doc.path)))
                .child(Self::library_menu(cx));
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
                        .child(if self.deleted_document.is_some() {
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
        let has_detail = self.current().is_some()
            || self.deleted_document.is_some()
            || self.page.is_some()
            || self.file_form.is_some();
        let can_back = has_detail || self.folders.len() > 1;
        #[cfg(target_os = "ios")]
        super::ios::update(
            can_back,
            self.current()
                .filter(|_| self.page.is_none() && self.file_form.is_none())
                .map(|d| d.handler.read(cx)),
            cx.theme().background,
        );
        let show_browser = self.file_form.is_none() && self.page.is_none() && (wide || !has_detail);
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
                                v.deleted_document = None;
                                v.back(w, cx);
                            })),
                    ),
                );
            }
            content = content.child(browser.child(folder.render(
                self,
                wide || !cfg!(target_os = "ios"),
                self.current().map(|d| d.path.as_path()),
                cx,
            )));
        }
        if let Some(form) = &self.file_form {
            content = content.child(self.render_form(form, cx));
        } else if let Some(page) = &self.page {
            content = content.child(self.render_page(*page, cx));
        } else if wide || has_detail {
            content = content.child(self.detail(wide, has_detail, cx));
        }
        panel = panel
            .overflow_hidden()
            .child(self.navigation_content(content, wide, cx));
        panel = panel.track_focus(&self.focus);
        panel = self.feedback(panel, cx);
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
