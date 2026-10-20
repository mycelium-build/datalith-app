use gpui_kit::{AppContext as _, Context, Focusable as _, Window};

use super::Tab;
use crate::ui::{DatalithView, shortcuts::ShortcutsView};

impl DatalithView {
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
            let view = cx.new(ShortcutsView::new);
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
        if self.tabs.remove(index) {
            self.focus_active_tab(window, cx);
            cx.notify();
        }
    }

    pub(crate) fn focus_active_tab(&self, window: &mut Window, cx: &mut Context<Self>) {
        match self.tabs.active() {
            Some(Tab::Document(tab)) => tab.handler.read(cx).focus_handle(cx).focus(window, cx),
            Some(Tab::Shortcuts(view)) => view.focus_handle(cx).focus(window, cx),
            None => {}
        }
    }
}
