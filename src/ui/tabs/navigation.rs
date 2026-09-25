use std::path::{Path, PathBuf};

use gpui_kit::component::{WindowExt, input::InputEvent};
use gpui_kit::{AppContext, Context, Window};
use percent_encoding::percent_decode_str;

use super::Tab;
use crate::app::workspace::{TabId, WorkspaceTab};
use crate::document::handler::{FileHandler, FileHandlerEvent, ViewMode};
use crate::document::registry::ViewerDependencies;
use crate::ui::DatalithView;
use crate::ui::notifications;
use crate::vault::file_ops;

enum OpenMode {
    Replace,
    NewTab,
    Created,
    Restore { id: TabId, mode: ViewMode },
    History { position: usize },
}

#[derive(Clone, Copy, Debug)]
pub enum NavigationAction {
    GoBack,
    GoForward,
}

impl DatalithView {
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
                Some(path) if path.is_file() && self.registry.is_supported(&path) => {
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
        if !self.registry.is_supported(&path) {
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
        let mode_for_handler = match mode {
            OpenMode::Replace | OpenMode::History { .. } => {
                current_mode.unwrap_or_else(|| crate::app::settings::snapshot().open_new_tab_mode())
            }
            OpenMode::NewTab => crate::app::settings::snapshot().open_new_tab_mode(),
            OpenMode::Created => ViewMode::Edit,
            OpenMode::Restore { mode, .. } => *mode,
        };
        let tab_id = match mode {
            OpenMode::Replace | OpenMode::History { .. } => self
                .tabs
                .active()
                .map_or_else(TabId::new, |tab| tab.id().clone()),
            OpenMode::NewTab | OpenMode::Created => TabId::new(),
            OpenMode::Restore { id, .. } => id.clone(),
        };

        let (history, history_position) = match mode {
            OpenMode::NewTab | OpenMode::Created | OpenMode::Restore { .. } => {
                (vec![path.clone()], 0)
            }
            OpenMode::Replace => self.tabs.active().map_or_else(
                || (vec![path.clone()], 0),
                |tab| next_history(&tab.history, tab.history_position, &path),
            ),
            OpenMode::History { position } => {
                let history = self
                    .tabs
                    .active()
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
        let input_subscription = handler.read(cx).input().cloned().map(|state| {
            let path = path.clone();
            cx.subscribe_in(&state, window, move |_view, state, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    let content = state.read(cx).value();
                    if let Err(error) = file_ops::update(&path, &content) {
                        window
                            .push_notification(notifications::save_file_failed(&path, &error), cx);
                    }
                }
            })
        });
        let event_subscription = cx.subscribe_in(
            &handler,
            window,
            move |view, _handler, event: &FileHandlerEvent, window, cx| match event {
                FileHandlerEvent::LinkClicked(url, new_tab) => {
                    let decoded_url = percent_decode_str(url).decode_utf8_lossy();
                    if let Some(ref catalog) = view.vault_catalog
                        && let Some(resolved) = catalog.resolve(&decoded_url)
                    {
                        view.open_file(resolved, *new_tab, window, cx);
                        return;
                    }
                    cx.open_url(url);
                }
            },
        );
        let tab = Tab {
            id: tab_id,
            path,
            handler,
            _input_subscription: input_subscription,
            _event_subscription: Some(event_subscription),
            history,
            history_position,
        };
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
        self.new_empty_tab_with_id(TabId::new(), ViewMode::Edit, cx);
    }

    fn new_empty_tab_with_id(&mut self, id: TabId, mode: ViewMode, cx: &mut Context<Self>) {
        let handler = cx.new(|_cx| FileHandler::new(mode, None, None));
        self.tabs.insert(
            Tab {
                id,
                path: PathBuf::new(),
                handler,
                _input_subscription: None,
                _event_subscription: None,
                history: Vec::new(),
                history_position: 0,
            },
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

    pub(crate) fn close_active_tab(&mut self, cx: &mut Context<Self>) {
        if let Some(index) = self.tabs.active_index() {
            self.close_tab(index, cx);
        }
    }

    pub(crate) fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.tabs.remove(index) {
            cx.notify();
        }
    }

    pub(crate) fn close_tabs_under(&mut self, root: &Path, cx: &mut Context<Self>) {
        let indices: Vec<_> = self
            .tabs
            .entries
            .iter()
            .enumerate()
            .filter(|(_, tab)| tab.path.starts_with(root))
            .map(|(index, _)| index)
            .collect();
        for index in indices.into_iter().rev() {
            self.tabs.remove(index);
        }
        cx.notify();
    }

    pub(crate) fn go_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.active() else {
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
        let Some(tab) = self.tabs.active() else {
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
            .active()
            .is_some_and(|tab| tab.history_position > 0)
    }

    pub(crate) fn can_go_forward(&self) -> bool {
        self.tabs
            .active()
            .is_some_and(|tab| tab.history_position.saturating_add(1) < tab.history.len())
    }

    pub(crate) fn focus_active_tab(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(handler) = self.tabs.active_handler() {
            handler.read(cx).focus_handle(cx).focus(window, cx);
        }
    }
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use gpui_kit::component::Root;
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{AppContext, TestAppContext, px, size};

    use super::{DatalithView, next_history};
    use crate::document::handler::ViewMode;
    use crate::ui::settings::SettingsView;

    #[test]
    fn navigation_preserves_tab_identity_modes_and_only_history_creates_duplicates() {
        let root = std::env::temp_dir().join(format!(
            "datalith-navigation-{:032x}",
            rand::random::<u128>()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let a = root.join("A.md");
        let b = root.join("B.md");
        let created = root.join("Created.md");
        for path in [&a, &b, &created] {
            std::fs::write(path, "# Note").unwrap();
        }
        let mut cx = TestAppContext::single();
        cx.update(|cx| {
            gpui_kit::init(cx);
            SettingsView::init_theme_options(cx);
        });
        crate::app::settings::set_open_new_tab_mode(ViewMode::View).unwrap();
        let window_handle = cx.open_window(size(px(1000.), px(700.)), |window, cx| {
            let view = cx.new(|cx| DatalithView::new(false, Vec::new(), window, cx));
            Root::new(view, window, cx)
        });
        cx.update_window(window_handle.into(), |root, window, cx| {
            let root = root.downcast::<Root>().unwrap();
            let view = root
                .read(cx)
                .view()
                .clone()
                .downcast::<DatalithView>()
                .unwrap();
            view.update(cx, |view, cx| {
                view.open_file(a.clone(), true, window, cx);
                let first = view.tabs.active_tab_id().unwrap().clone();
                view.open_file(b.clone(), false, window, cx);
                assert_eq!(view.tabs.active_tab_id(), Some(&first));
                assert_eq!(
                    view.tabs.active_handler().unwrap().read(cx).mode(),
                    ViewMode::View
                );
                crate::app::settings::set_open_new_tab_mode(ViewMode::Edit).unwrap();
                view.open_file(a.clone(), true, window, cx);
                let second = view.tabs.active_tab_id().unwrap().clone();
                assert_ne!(first, second);
                assert_eq!(
                    view.tabs.active_handler().unwrap().read(cx).mode(),
                    ViewMode::Edit
                );
                view.open_file(b.clone(), true, window, cx);
                assert_eq!(view.tabs.active_tab_id(), Some(&first));
                view.go_back(window, cx);
                assert_eq!(view.tabs.open_paths(), vec![a.clone(), a.clone()]);
                assert_eq!(view.tabs.active_tab_id(), Some(&first));
                assert_eq!(
                    view.tabs.active_handler().unwrap().read(cx).mode(),
                    ViewMode::View
                );
                view.open_file(a.clone(), true, window, cx);
                assert_eq!(view.tabs.active_tab_id(), Some(&first));
                view.go_forward(window, cx);
                assert_eq!(view.tabs.active_path(), Some(b.as_path()));
                assert_eq!(view.tabs.active_tab_id(), Some(&first));
                view.go_back(window, cx);
                let saved = view.tabs.snapshot(cx);
                assert_eq!(
                    saved.0.iter().map(|tab| tab.mode).collect::<Vec<_>>(),
                    vec![ViewMode::View, ViewMode::Edit]
                );
                view.restore_workspace_tabs(saved.0.clone(), saved.1.as_ref(), window, cx);
                assert_eq!(view.tabs.snapshot(cx), saved);
                assert!(!view.can_go_back());
                crate::app::settings::set_open_new_tab_mode(ViewMode::View).unwrap();
                view.open_created_file(created.clone(), window, cx);
                let created_id = view.tabs.active_tab_id().unwrap().clone();
                assert_eq!(
                    view.tabs.active_handler().unwrap().read(cx).mode(),
                    ViewMode::Edit
                );
                view.close_active_tab(cx);
                view.open_file(created.clone(), true, window, cx);
                assert_ne!(view.tabs.active_tab_id(), Some(&created_id));
                assert_eq!(
                    view.tabs.active_handler().unwrap().read(cx).mode(),
                    ViewMode::View
                );
            });
        })
        .unwrap();
        finish_startup(&mut cx, window_handle);
        drop(cx);
        std::fs::remove_dir_all(root).unwrap();
    }

    fn finish_startup(cx: &mut TestAppContext, window_handle: gpui_kit::WindowHandle<Root>) {
        cx.update_window(window_handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.press("escape", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_secs(6));
        cx.run_until_parked();
        cx.update_window(window_handle.into(), |_, window, cx| {
            window.render_frame(cx);
        })
        .unwrap();
    }

    #[test]
    fn history_appends_replaces_current_and_branches_after_back() {
        let history = vec![PathBuf::from("a.md")];
        let (history, position) = next_history(&history, 0, PathBuf::from("b.md").as_path());
        assert_eq!(history, [PathBuf::from("a.md"), PathBuf::from("b.md")]);
        assert_eq!(position, 1);

        let (same_history, same_position) =
            next_history(&history, 1, PathBuf::from("b.md").as_path());
        assert_eq!(same_history, history);
        assert_eq!(same_position, 1);

        let (branched, branch_position) = next_history(
            &["a.md", "b.md", "c.md"].map(PathBuf::from),
            0,
            PathBuf::from("d.md").as_path(),
        );
        assert_eq!(branched, [PathBuf::from("a.md"), PathBuf::from("d.md")]);
        assert_eq!(branch_position, 1);
    }
}
