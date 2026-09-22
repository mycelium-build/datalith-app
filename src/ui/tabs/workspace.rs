use gpui_kit::{AppContext as _, Context, DismissEvent, Focusable as _, Window};

use super::Tab;
use crate::ui::{DatalithView, shortcuts::ShortcutsView, themes::ThemeEditor};

impl DatalithView {
    pub(crate) fn open_theme_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.close();
        if let Some(index) = self
            .tabs
            .entries
            .iter()
            .position(|tab| matches!(tab, Tab::Theme { .. }))
        {
            self.tabs.select(index);
        } else {
            let editor = cx.new(|cx| ThemeEditor::new(window, cx));
            let dismiss = cx.subscribe_in(
                &editor,
                window,
                |view, editor, _: &DismissEvent, window, cx| {
                    if let Some(index) = view
                        .tabs
                        .entries
                        .iter()
                        .position(|tab| tab.entity_id() == editor.entity_id())
                    {
                        view.tabs.remove(index);
                        view.focus_active_tab(window, cx);
                        cx.notify();
                    }
                },
            );
            let change = cx.observe(&editor, |_, _, cx| cx.notify());
            self.tabs.insert(
                Tab::Theme {
                    editor,
                    _dismiss_subscription: dismiss,
                    _change_subscription: change,
                },
                true,
            );
        }
        self.focus_active_tab(window, cx);
        cx.notify();
    }

    pub(crate) fn open_shortcuts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.close();
        if let Some(index) = self
            .tabs
            .entries
            .iter()
            .position(|tab| matches!(tab, Tab::Shortcuts(_)))
        {
            self.tabs.select(index);
        } else {
            let view = cx.new(|cx| ShortcutsView::new(cx));
            self.tabs.insert(Tab::Shortcuts(view), true);
        }
        self.focus_active_tab(window, cx);
        cx.notify();
    }

    pub(crate) fn close_active_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.tabs.active_index() {
            self.close_tab(index, window, cx);
        }
    }

    pub(crate) fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(Tab::Theme { editor, .. }) = self.tabs.entries.get(index) {
            let editor = editor.clone();
            if editor.read(cx).has_unsaved_changes() {
                self.tabs.select(index);
            }
            editor.update(cx, |editor, cx| editor.request_close(window, cx));
            cx.notify();
        } else if self.tabs.remove(index) {
            self.focus_active_tab(window, cx);
            cx.notify();
        }
    }

    pub(crate) fn focus_active_tab(&self, window: &mut Window, cx: &mut Context<Self>) {
        match self.tabs.active() {
            Some(Tab::Document(tab)) => tab.handler.read(cx).focus_handle(cx).focus(window, cx),
            Some(Tab::Theme { editor, .. }) => editor.focus_handle(cx).focus(window, cx),
            Some(Tab::Shortcuts(view)) => view.focus_handle(cx).focus(window, cx),
            None => {}
        }
    }
}
