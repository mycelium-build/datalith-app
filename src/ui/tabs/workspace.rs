use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{AppContext as _, Context, Focusable as _, Window};

use super::Tab;
use crate::ui::{DatalithView, shortcuts::ShortcutsView, themes::ThemeEditor};

impl DatalithView {
    pub(crate) fn open_theme_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = if cx.theme().is_dark() {
            crate::app::settings::ThemeKind::Dark
        } else {
            crate::app::settings::ThemeKind::Light
        };
        let name = cx
            .global::<crate::app::themes::ThemeLibrary>()
            .current(current)
            .to_owned();
        let family = cx
            .global::<crate::app::themes::ThemeLibrary>()
            .families()
            .find(|f| f.variants().iter().any(|v| v.name() == name));
        if let Some(family) = family {
            let id = family.id();
            let variant = family
                .variants()
                .iter()
                .find(|v| v.name() == name)
                .map(crate::app::themes::ThemeVariant::id);
            if matches!(family.source(), crate::app::themes::ThemeSource::Custom(_)) {
                self.open_theme_editor_for(id, variant, window, cx);
                return;
            }
        }
        self.settings.open_theme();
        self.settings.focus(window, cx);
        cx.notify();
    }

    pub(crate) fn open_theme_editor_for(
        &mut self,
        family_id: u64,
        variant: Option<u64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !cx
            .global::<crate::app::themes::ThemeLibrary>()
            .family(family_id)
            .is_some_and(|family| {
                matches!(family.source(), crate::app::themes::ThemeSource::Custom(_))
            })
        {
            return;
        }
        self.settings.close();
        if let Some(index) = self
            .tabs
            .entries
            .iter()
            .position(|tab| matches!(tab, Tab::Theme { editor, .. } if editor.read(cx).family_id() == family_id))
        {
            self.tabs.select(index);
            if let Some(variant) = variant && let Some(Tab::Theme { editor, .. }) = self.tabs.active() {
                editor.update(cx, |editor, cx| editor.select(variant, window, cx));
            }
        } else {
            let editor = cx.new(|cx| ThemeEditor::new(family_id, variant, window, cx));
            let change = cx.observe(&editor, |_, _, cx| cx.notify());
            self.tabs.insert(
                Tab::Theme {
                    editor,
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
