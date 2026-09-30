use gpui_kit::component::WindowExt as _;
use gpui_kit::{AppContext as _, Context, Focusable as _, Window};

use super::Tab;
use crate::ui::{DatalithView, notifications, shortcuts::ShortcutsView, themes::ThemeEditor};

impl DatalithView {
    pub(crate) fn open_themes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.open_theme(window, cx);
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
        if !matches!(family.source(), crate::app::themes::ThemeSource::Custom(_)) {
            return;
        }
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
            let editor = match ThemeEditor::try_new(family_id, variant, window, cx) {
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
        let Some(index) = self.tabs.entries.iter().position(
            |tab| matches!(tab, Tab::Theme { editor, .. } if editor.read(cx).family_id() == family_id),
        ) else {
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

#[cfg(test)]
mod tests {
    use gpui_kit::component::Root;
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{AppContext as _, TestAppContext, px, size};

    use crate::app::{
        AppState,
        fonts::FontCatalog,
        themes::{ImportPolicy, ThemeLibrary},
    };
    use crate::ui::{DatalithView, settings::theme::show_undo};

    #[test]
    fn deleting_a_family_or_its_last_variant_closes_the_editor_without_undo_reopening_it() {
        for last_variant in [false, true] {
            assert_deletion_closes_editor(last_variant);
        }
    }

    fn assert_deletion_closes_editor(last_variant: bool) {
        let root = std::env::temp_dir().join(format!(
            "datalith-theme-tabs-{:032x}",
            rand::random::<u128>()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("import.json");
        std::fs::write(&file, r##"{"name":"Test family","themes":[{"name":"Test family Light","mode":"light","colors":{"background":"#abcdef"}}]}"##).unwrap();
        let mut cx = TestAppContext::single();
        cx.update(|cx| {
            gpui_kit::init(cx);
            FontCatalog::init(cx);
            let mut library = ThemeLibrary::new(root.join("themes"));
            let defaults = library
                .prepare_import(std::path::Path::new(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/assets/themes/datalith.json"
                )))
                .unwrap();
            library
                .import_prepared(&defaults, ImportPolicy::Copy)
                .unwrap();
            cx.set_global(library);
            cx.set_global(AppState::default());
        });
        let handle = cx.open_window(size(px(1000.), px(700.)), |window, cx| {
            let view = cx.new(|cx| DatalithView::new(false, Vec::new(), window, cx));
            cx.global_mut::<AppState>().view = Some(view.clone());
            Root::new(view, window, cx)
        });
        let id = cx.update(|cx| {
            let library = cx.global_mut::<ThemeLibrary>();
            let prepared = library.prepare_import(&file).unwrap();
            library
                .import_prepared(&prepared, ImportPolicy::Copy)
                .unwrap()
        });
        cx.update_window(handle.into(), |_, window, cx| {
            let view = cx.global::<AppState>().view.clone().unwrap();
            view.update(cx, |view, cx| {
                view.open_shortcuts(window, cx);
                view.open_theme_editor_for(id, None, window, cx);
            });
            assert!(view.read(cx).tabs.theme_editor_for(id, cx).is_some());
            let library = cx.global_mut::<ThemeLibrary>();
            let deleted = if last_variant {
                let variant = library.family(id).unwrap().variants().first().unwrap().id();
                library.remove_variant(variant).unwrap()
            } else {
                library.delete_family(id).unwrap()
            };
            show_undo(id, deleted, window, cx);
        })
        .unwrap();
        cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
            .unwrap();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(200));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            let view = cx.global::<AppState>().view.clone().unwrap();
            assert!(view.read(cx).tabs.theme_editor_for(id, cx).is_none());
            assert!(matches!(
                view.read(cx).tabs.active(),
                Some(super::Tab::Shortcuts(_))
            ));
            window.render_frame(cx);
            window.click("undo-theme-delete", cx);
        })
        .unwrap();
        cx.update(|cx| {
            let view = cx.global::<AppState>().view.clone().unwrap();
            assert!(
                cx.global::<ThemeLibrary>().family(id).is_some(),
                "last_variant={last_variant}"
            );
            assert!(view.read(cx).tabs.theme_editor_for(id, cx).is_none());
        });
        cx.executor()
            .advance_clock(std::time::Duration::from_secs(6));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
            .unwrap();
        drop(cx);
        std::fs::remove_dir_all(root).unwrap();
    }
}
