use std::path::{Path, PathBuf};

use gpui::{AppContext, Context, Window};
use gpui_component::input::InputEvent;
use percent_encoding::percent_decode_str;

use super::Tab;
use crate::document::handler::{FileHandler, FileHandlerEvent, ViewMode};
use crate::document::registry::ViewerDependencies;
use crate::ui::DatalithView;
use crate::vault::file_ops;

#[derive(Clone, Copy)]
enum OpenMode {
    Replace,
    NewTab,
    History { position: usize },
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
        self.open_file_with_mode(path, mode, window, cx);
    }

    fn open_file_with_mode(
        &mut self,
        path: PathBuf,
        mode: OpenMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.registry.is_supported(&path) {
            return;
        }

        if !matches!(mode, OpenMode::History { .. })
            && let Some(index) = self.tabs.find_path(&path)
        {
            self.tabs.select(index);
            self.focus_active_tab(window, cx);
            cx.notify();
            return;
        }

        let (history, history_position) = match mode {
            OpenMode::NewTab => (vec![path.clone()], 0),
            OpenMode::Replace => self.tabs.active().map_or_else(
                || (vec![path.clone()], 0),
                |tab| next_history(&tab.history, tab.history_position, &path),
            ),
            OpenMode::History { position } => {
                let history = self
                    .tabs
                    .active()
                    .map_or_else(|| vec![path.clone()], |tab| tab.history.clone());
                (history, position)
            }
        };

        let dependencies = ViewerDependencies::new(self.vault_catalog.clone());
        let handler = cx.new(|cx| {
            self.registry
                .create_handler(&path, &dependencies, window, cx)
        });
        let input_subscription = handler.read(cx).input().cloned().map(|state| {
            let path = path.clone();
            cx.subscribe_in(&state, window, move |_view, state, event, _window, cx| {
                if matches!(event, InputEvent::Change) {
                    let content = state.read(cx).value();
                    let _ = file_ops::update(&path, &content);
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
            path,
            handler,
            _input_subscription: input_subscription,
            _event_subscription: Some(event_subscription),
            history,
            history_position,
        };
        self.tabs.insert(tab, matches!(mode, OpenMode::NewTab));
        self.focus_active_tab(window, cx);
        cx.notify();
    }

    pub(crate) fn new_empty_tab(&mut self, cx: &mut Context<Self>) {
        let handler = cx.new(|_cx| FileHandler::new(ViewMode::Edit, None, None));
        self.tabs.insert(
            Tab {
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
        self.open_file_with_mode(path, OpenMode::History { position }, window, cx);
    }

    pub(crate) fn go_forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.active() else {
            return;
        };
        let position = tab.history_position.saturating_add(1);
        let Some(path) = tab.history.get(position).cloned() else {
            return;
        };
        self.open_file_with_mode(path, OpenMode::History { position }, window, cx);
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
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::indexing_slicing,
        clippy::string_slice
    )]
    use super::next_history;
    use std::path::PathBuf;

    #[test]
    fn replacing_appends_to_navigation_history() {
        let history = vec![PathBuf::from("a.md")];
        let (history, position) = next_history(&history, 0, PathBuf::from("b.md").as_path());
        assert_eq!(history, [PathBuf::from("a.md"), PathBuf::from("b.md")]);
        assert_eq!(position, 1);
    }

    #[test]
    fn replacing_after_back_discards_forward_history() {
        let history = vec!["a.md", "b.md", "c.md"]
            .into_iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        let (history, position) = next_history(&history, 0, PathBuf::from("d.md").as_path());
        assert_eq!(history, [PathBuf::from("a.md"), PathBuf::from("d.md")]);
        assert_eq!(position, 1);
    }

    #[test]
    fn replacing_with_current_path_is_a_noop() {
        let history = vec![PathBuf::from("a.md"), PathBuf::from("b.md")];
        let (updated, position) = next_history(&history, 1, PathBuf::from("b.md").as_path());
        assert_eq!(updated, history);
        assert_eq!(position, 1);
    }
}
