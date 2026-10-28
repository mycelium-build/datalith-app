use gpui_kit::component::WindowExt as _;
use gpui_kit::{AppContext as _, Context, Focusable as _, Window};

use super::Tab;
use crate::ui::{DatalithView, notifications, shortcuts::ShortcutsView, themes::ThemeEditor};

impl DatalithView {
    pub(crate) fn open_themes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.open_theme();
        self.settings.focus(window, cx);
        cx.notify();
    }

    pub(crate) fn open_theme_editor_for(
        &mut self,
        family_id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(family) = cx
            .global::<crate::app::themes::ThemeLibrary>()
            .family(family_id)
        else {
            window.push_notification(
                notifications::theme_editor_open_failed(&anyhow::anyhow!(
                    "Theme family no longer exists"
                )),
                cx,
            );
            return;
        };
        if !family.is_custom() {
            return;
        }
        if let Some(index) = self.tabs.theme_editor_index(family_id, cx) {
            self.tabs.select(index);
        } else {
            let editor = match ThemeEditor::try_new(family_id, window, cx) {
                Ok(editor) => editor,
                Err(error) => {
                    window.push_notification(notifications::theme_editor_open_failed(&error), cx);
                    return;
                }
            };
            let change = cx.observe(&editor, |_, _, cx| cx.notify());
            self.tabs.insert(
                Tab::Theme {
                    editor,
                    _change_subscription: change,
                },
                true,
            );
        }
        self.settings.close();
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

    pub(crate) fn close_theme_editor(
        &mut self,
        family_id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.tabs.theme_editor_index(family_id, cx) else {
            return;
        };
        let was_active = self.tabs.active_index() == Some(index);
        self.tabs.remove(index);
        if was_active && !self.settings.open {
            self.focus_active_tab(window, cx);
        }
        cx.notify();
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
            Some(Tab::Theme { editor, .. }) => editor.focus_handle(cx).focus(window, cx),
            Some(Tab::Shortcuts(view)) => view.focus_handle(cx).focus(window, cx),
            None => {}
        }
    }
}
