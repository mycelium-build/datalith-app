use gpui::*;
use gpui_component::{
    ActiveTheme, IconName, Sizable, Size,
    button::{Button, ButtonVariants as _},
    h_flex,
    scroll::ScrollableElement,
    setting::{SettingField, SettingGroup, SettingItem, SettingPage, Settings},
    v_flex,
};

use crate::config::{save_dark_theme_name, save_light_theme_name};

use super::DatalithView;

#[derive(Clone)]
pub(crate) struct ThemeOptions {
    pub light_theme_name: SharedString,
    pub dark_theme_name: SharedString,
    pub theme_options: Vec<(SharedString, SharedString)>,
}

impl Global for ThemeOptions {}

pub(crate) struct SettingsView {
    pub(crate) open: bool,
}

impl SettingsView {
    pub(crate) fn new() -> Self {
        Self { open: false }
    }

    pub(crate) fn open(&mut self) {
        self.open = true;
    }

    pub(crate) fn close(&mut self) {
        self.open = false;
    }

    pub(crate) fn init_theme_options(cx: &mut App) {
        let registry = gpui_component::ThemeRegistry::global(cx);
        let mut theme_options: Vec<(SharedString, SharedString)> = registry
            .themes()
            .keys()
            .map(|n| (n.clone(), n.clone()))
            .collect();
        theme_options.sort_by_key(|(n, _)| n.to_lowercase());

        let saved_light = crate::config::load_light_theme_name()
            .unwrap_or_default()
            .into();
        let saved_dark = crate::config::load_dark_theme_name()
            .unwrap_or_default()
            .into();

        cx.set_global(ThemeOptions {
            light_theme_name: saved_light,
            dark_theme_name: saved_dark,
            theme_options,
        });
    }

    pub(crate) fn render_overlay(&self, cx: &mut Context<DatalithView>) -> impl IntoElement {
        let theme_options = cx.global::<ThemeOptions>().theme_options.clone();

        div()
            .absolute()
            .inset_0()
            .bg(gpui::black().opacity(0.3))
            .flex()
            .items_center()
            .justify_center()
            .id("settings-backdrop")
            .on_click(cx.listener(|view: &mut DatalithView, _, _, cx| {
                view.settings.close();
                cx.notify();
            }))
            .child(
                div()
                    .w(px(700.))
                    .h(px(600.))
                    .bg(cx.theme().background)
                    .border(px(1.))
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .shadow_lg()
                    .id("settings-panel")
                    .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()))
                    .on_key_down(cx.listener(|view: &mut DatalithView, event: &KeyDownEvent, _, cx| {
                        if event.keystroke.key == "escape" {
                            view.settings.close();
                            cx.notify();
                        }
                    }))
                    .child(
                        v_flex()
                            .size_full()
                            .overflow_hidden()
                            .child(
                                h_flex()
                                    .w_full()
                                    .p_2()
                                    .justify_between()
                                    .border_b(px(1.))
                                    .border_color(cx.theme().border)
                                    .child("Settings")
                                    .child(
                                        Button::new("close-settings")
                                            .ghost()
                                            .small()
                                            .icon(IconName::Close)
                                            .on_click(cx.listener(|view, _, _, cx| {
                                                view.settings.close();
                                                cx.notify();
                                            })),
                                    ),
                            )
                    .child(
                        div()
                            .flex_1()
                            .overflow_y_scrollbar()
                            .p_4()
                                    .child(
                                        Settings::new("app-settings")
                                            .with_size(Size::Small)
                                            .pages(vec![SettingPage::new("Appearance")
                                                .default_open(true)
                                                .groups(vec![SettingGroup::new()
                                                    .title("Theme")
                                                    .items(vec![
                                                        SettingItem::new(
                                                            "Light Theme",
                                                            SettingField::scrollable_dropdown(
                                                                theme_options.clone(),
                                                                |cx| cx.global::<ThemeOptions>().light_theme_name.clone(),
                                                                |val: SharedString, cx| {
                                                                    cx.global_mut::<ThemeOptions>().light_theme_name =
                                                                        val.clone();
                                                                    let registry =
                                                                        gpui_component::ThemeRegistry::global(cx);
                                                                    if let Some(theme_config) =
                                                                        registry.themes().get(val.as_str())
                                                                    {
                                                                        gpui_component::Theme::global_mut(cx).light_theme =
                                                                            theme_config.clone();
                                                                        let current_mode =
                                                                            gpui_component::Theme::global(cx).mode;
                                                                        gpui_component::Theme::change(
                                                                            current_mode, None, cx,
                                                                        );
                                                                    }
                                                                    cx.refresh_windows();
                                                                    let _ = save_light_theme_name(&val);
                                                                },
                                                            ),
                                                        )
                                                        .description("Theme used in light mode."),
                                                        SettingItem::new(
                                                            "Dark Theme",
                                                            SettingField::scrollable_dropdown(
                                                                theme_options.clone(),
                                                                |cx| cx.global::<ThemeOptions>().dark_theme_name.clone(),
                                                                |val: SharedString, cx| {
                                                                    cx.global_mut::<ThemeOptions>().dark_theme_name =
                                                                        val.clone();
                                                                    let registry =
                                                                        gpui_component::ThemeRegistry::global(cx);
                                                                    if let Some(theme_config) =
                                                                        registry.themes().get(val.as_str())
                                                                    {
                                                                        gpui_component::Theme::global_mut(cx).dark_theme =
                                                                            theme_config.clone();
                                                                        let current_mode =
                                                                            gpui_component::Theme::global(cx).mode;
                                                                        gpui_component::Theme::change(
                                                                            current_mode, None, cx,
                                                                        );
                                                                    }
                                                                    cx.refresh_windows();
                                                                    let _ = save_dark_theme_name(&val);
                                                                },
                                                            ),
                                                        )
                                                        .description("Theme used in dark mode."),
                                                    ])])]),
                                    ),
                            ),
                    ),
            )
    }
}
