use std::path::{Path, PathBuf};

use gpui_kit::component::{WindowExt, input::InputEvent};
use gpui_kit::{AppContext, Context, Window};
use percent_encoding::percent_decode_str;

use super::{DocumentTab, Tab};
use crate::app::system;
use crate::app::workspace::{TabId, WorkspaceTab};
use crate::document::handler::{FileHandler, FileHandlerEvent, ViewMode};
use crate::document::markdown::is_external_url;
use crate::document::registry::ViewerDependencies;
use crate::ui::DatalithView;
use crate::ui::notifications;
use crate::vault::CatalogState;
use crate::vault::VaultCatalog;
use crate::vault::file_ops;
use crate::vault::path::{path_to_vault_relative, resolve_relative_link};

enum OpenMode {
    Replace,
    NewTab,
    Created,
    Restore { id: TabId, mode: ViewMode },
    History { position: usize },
}

impl OpenMode {
    fn view_mode(&self, current: Option<ViewMode>) -> ViewMode {
        match self {
            Self::Replace | Self::History { .. } => {
                current.unwrap_or_else(|| crate::app::settings::snapshot().open_new_tab_mode())
            }
            Self::NewTab => crate::app::settings::snapshot().open_new_tab_mode(),
            Self::Created => ViewMode::Edit,
            Self::Restore { mode, .. } => *mode,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum NavigationAction {
    GoBack,
    GoForward,
}

impl DatalithView {
    fn open_link_target(
        &mut self,
        url: &str,
        new_tab: bool,
        source: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = link_target(url);
        let target = target.as_str();
        // A fragment-only link points inside the current document.
        if target.is_empty() {
            return;
        }
        let explicitly_relative = target.starts_with("./") || target.starts_with("../");
        let root = self.root_path.as_deref();
        let relative = root.and_then(|root| resolve_relative_link(source, target, root));
        // ./ / ../ that exists on disk -> open.
        if explicitly_relative
            && let Some(resolved) = relative.as_ref().filter(|path| self.is_openable(path))
        {
            self.open_file(resolved.clone(), new_tab, window, cx);
            return;
        }
        // ./ / ../ extensionless -> catalog, scoped by the relative folder.
        if explicitly_relative
            && let Some(relative_target) = root
                .zip(relative.as_deref())
                .and_then(|(root, path)| path_to_vault_relative(path, root).ok().flatten())
            && let Some(ref catalog) = self.vault_catalog
            && let Some(resolved) = catalog.resolve(&relative_target)
        {
            self.open_file(resolved, new_tab, window, cx);
            return;
        }
        // Vault-wide catalog resolution.
        if let Some(ref catalog) = self.vault_catalog
            && let Some(resolved) = catalog.resolve(target)
        {
            self.open_file(resolved, new_tab, window, cx);
            return;
        }
        // Plain relative existing file (catalog miss).
        if let Some(resolved) = relative.filter(|path| self.is_openable(path)) {
            self.open_file(resolved, new_tab, window, cx);
            return;
        }
        // External URL -> system opener, notify on command failure.
        if is_external_url(target) {
            if let Err(error) = system::open_url(url) {
                window.push_notification(notifications::open_url_failed(url, &error), cx);
            }
        } else {
            let notification = match self.vault_catalog.as_ref().map(VaultCatalog::state) {
                Some(CatalogState::Ready) => notifications::link_target_not_found(url),
                Some(CatalogState::Failed) => notifications::link_target_catalog_failed(),
                Some(CatalogState::Syncing) => notifications::link_target_while_loading(),
                None => notifications::link_target_no_vault(),
            };
            window.push_notification(notification, cx);
        }
    }

    fn is_openable(&self, path: &Path) -> bool {
        crate::vault::source::is_file(path) && self.registry.is_supported(path)
    }

    pub(crate) fn open_file(
        &mut self,
        path: PathBuf,
        new_tab: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mode = if new_tab {
            OpenMode::NewTab
        } else {
            OpenMode::Replace
        };
        self.open_file_with_mode(path, &mode, window, cx);
    }

    pub(crate) fn open_created_file(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_file_with_mode(path, &OpenMode::Created, window, cx);
    }

    pub(crate) fn restore_workspace_tabs(
        &mut self,
        tabs: Vec<WorkspaceTab>,
        active_id: Option<&TabId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.tabs.clear();
        for tab in tabs {
            match tab.path {
                Some(path)
                    if crate::vault::source::is_file(&path)
                        && self.registry.is_supported(&path) =>
                {
                    self.open_file_with_mode(
                        path,
                        &OpenMode::Restore {
                            id: tab.id,
                            mode: tab.mode,
                        },
                        window,
                        cx,
                    );
                }
                Some(_) => {}
                None => self.new_empty_tab_with_id(tab.id, tab.mode, cx),
            }
        }

        if !active_id.is_some_and(|id| self.tabs.select_by_id(id)) && !self.tabs.is_empty() {
            self.tabs.select(0);
        }
        self.focus_active_tab(window, cx);
        cx.notify();
    }

    fn open_file_with_mode(
        &mut self,
        path: PathBuf,
        mode: &OpenMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.registry.is_supported(&path) || !crate::vault::source::is_file(&path) {
            return;
        }

        if matches!(mode, OpenMode::Replace | OpenMode::NewTab)
            && let Some(index) = self.tabs.find_path(&path)
        {
            self.tabs.select(index);
            self.focus_active_tab(window, cx);
            cx.notify();
            return;
        }

        let current_mode = self
            .tabs
            .active_handler()
            .map(|handler| handler.read(cx).mode());
        let mode_for_handler = mode.view_mode(current_mode);
        let tab_id = match mode {
            OpenMode::Replace | OpenMode::History { .. } => self
                .tabs
                .active_document()
                .map_or_else(TabId::new, |tab| tab.id().clone()),
            OpenMode::NewTab | OpenMode::Created => TabId::new(),
            OpenMode::Restore { id, .. } => id.clone(),
        };

        let (history, history_position) = match mode {
            OpenMode::NewTab | OpenMode::Created | OpenMode::Restore { .. } => {
                (vec![path.clone()], 0)
            }
            OpenMode::Replace => self.tabs.active_document().map_or_else(
                || (vec![path.clone()], 0),
                |tab| next_history(&tab.history, tab.history_position, &path),
            ),
            OpenMode::History { position } => {
                let history = self
                    .tabs
                    .active_document()
                    .map_or_else(|| vec![path.clone()], |tab| tab.history.clone());
                (history, *position)
            }
        };

        let dependencies = ViewerDependencies::new(self.vault_catalog.clone());
        let handler = cx.new(|cx| {
            self.registry
                .create_handler(&path, &dependencies, window, cx)
        });
        handler.update(cx, |handler, cx| handler.set_mode(mode_for_handler, cx));
        let input_subscription = if handler.read(cx).is_read_only() {
            None
        } else {
            handler.read(cx).input().cloned().map(|state| {
                let path = path.clone();
                cx.subscribe_in(&state, window, move |_view, state, event, window, cx| {
                    if !matches!(event, InputEvent::Change) {
                        return;
                    }
                    let content = state.read(cx).value();
                    let Err(error) = file_ops::update(&path, &content) else {
                        return;
                    };
                    window.push_notification(notifications::save_file_failed(&path, &error), cx);
                })
            })
        };
        let link_source = path.clone();
        let event_subscription = cx.subscribe_in(
            &handler,
            window,
            move |view, _handler, event: &FileHandlerEvent, window, cx| match event {
                FileHandlerEvent::LinkClicked(url, new_tab) => {
                    view.open_link_target(url, *new_tab, &link_source, window, cx);
                }
            },
        );
        let tab = Tab::Document(DocumentTab {
            id: tab_id,
            path: Some(path),
            handler,
            _input_subscription: input_subscription,
            _event_subscription: Some(event_subscription),
            history,
            history_position,
        });
        self.tabs.insert(
            tab,
            matches!(
                mode,
                OpenMode::NewTab | OpenMode::Created | OpenMode::Restore { .. }
            ),
        );
        self.focus_active_tab(window, cx);
        cx.notify();
    }

    pub(crate) fn new_empty_tab(&mut self, cx: &mut Context<Self>) {
        let mode = if self
            .root_path
            .as_deref()
            .is_some_and(crate::vault::source::is_read_only)
        {
            ViewMode::View
        } else {
            ViewMode::Edit
        };
        self.new_empty_tab_with_id(TabId::new(), mode, cx);
    }

    fn new_empty_tab_with_id(&mut self, id: TabId, mode: ViewMode, cx: &mut Context<Self>) {
        let read_only = self
            .root_path
            .as_deref()
            .is_some_and(crate::vault::source::is_read_only);
        let mode = if read_only { ViewMode::View } else { mode };
        let handler = cx.new(|_cx| FileHandler::new(mode, None, None).with_read_only(read_only));
        self.tabs.insert(
            Tab::Document(DocumentTab {
                id,
                path: None,
                handler,
                _input_subscription: None,
                _event_subscription: None,
                history: Vec::new(),
                history_position: 0,
            }),
            true,
        );
        cx.notify();
    }

    pub(crate) fn toggle_editor_mode(&mut self, cx: &mut Context<Self>) {
        let Some(handler) = self.tabs.active_handler() else {
            return;
        };
        let handler = handler.clone();
        if !handler.read(cx).can_toggle_mode() {
            return;
        }
        let mode = if handler.read(cx).is_editing() {
            ViewMode::View
        } else {
            ViewMode::Edit
        };
        handler.update(cx, |handler, cx| handler.set_mode(mode, cx));
        self.focus_editor_requested = true;
        cx.notify();
    }

    pub(crate) fn close_tabs_under(&mut self, root: &Path, cx: &mut Context<Self>) {
        let indices: Vec<_> = self
            .tabs
            .entries
            .iter()
            .enumerate()
            .filter(|(_, tab)| {
                tab.document()
                    .and_then(|tab| tab.path.as_ref())
                    .is_some_and(|path| path.starts_with(root))
            })
            .map(|(index, _)| index)
            .collect();
        for index in indices.into_iter().rev() {
            self.tabs.remove(index);
        }
        cx.notify();
    }

    pub(crate) fn go_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.active_document() else {
            return;
        };
        let Some(position) = tab.history_position.checked_sub(1) else {
            return;
        };
        let Some(path) = tab.history.get(position).cloned() else {
            return;
        };
        self.open_file_with_mode(path, &OpenMode::History { position }, window, cx);
    }

    pub(crate) fn go_forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.active_document() else {
            return;
        };
        let position = tab.history_position.saturating_add(1);
        let Some(path) = tab.history.get(position).cloned() else {
            return;
        };
        self.open_file_with_mode(path, &OpenMode::History { position }, window, cx);
    }

    pub(crate) fn can_go_back(&self) -> bool {
        self.tabs
            .active_document()
            .is_some_and(|tab| tab.history_position > 0)
    }

    pub(crate) fn can_go_forward(&self) -> bool {
        self.tabs
            .active_document()
            .is_some_and(|tab| tab.history_position.saturating_add(1) < tab.history.len())
    }
}

/// Strips the fragment/query from `url` and percent-decodes the remaining path.
///
/// The delimiters are structural in the raw URL,
/// so they are split off before decoding;
/// a percent-encoded `#` or `?` is part of the file name.
fn link_target(url: &str) -> String {
    let raw_target = url.split_once(['#', '?']).map_or(url, |(target, _)| target);
    percent_decode_str(raw_target)
        .decode_utf8_lossy()
        .trim()
        .to_owned()
}

fn next_history(history: &[PathBuf], position: usize, path: &Path) -> (Vec<PathBuf>, usize) {
    let mut history = history.to_vec();
    if history.get(position).is_some_and(|current| current == path) {
        return (history, position);
    }
    history.truncate(position.saturating_add(1));
    history.push(path.to_path_buf());
    let position = history.len().saturating_sub(1);
    (history, position)
}
