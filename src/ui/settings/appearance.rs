//! Appearance settings page: theme mode, light/dark themes, and font scale.

use gpui_kit::component::{
    h_flex,
    setting::{SettingField, SettingGroup, SettingItem},
    slider::{Slider, SliderState},
};
use gpui_kit::{App, Context, Entity, IntoElement, ParentElement, SharedString, Styled, div};

use super::{DatalithView, SettingsView, ThemeOptions};
use crate::app::{
    preferences,
    settings::{self, ThemeKind, ThemePreference},
};
use crate::ui::notifications;

impl SettingsView {
    pub(crate) fn init_theme_options(cx: &mut App) {
        let registry = gpui_kit::component::ThemeRegistry::global(cx);
        let mut light_options: Vec<(SharedString, SharedString)> = registry
            .themes()
            .iter()
            .filter(|(_, theme)| theme.mode == gpui_kit::component::ThemeMode::Light)
            .map(|(name, _)| (name.clone(), name.clone()))
            .collect();
        light_options.sort_by_key(|(name, _)| name.to_lowercase());
        let mut dark_options: Vec<(SharedString, SharedString)> = registry
            .themes()
            .iter()
            .filter(|(_, theme)| theme.mode == gpui_kit::component::ThemeMode::Dark)
            .map(|(name, _)| (name.clone(), name.clone()))
            .collect();
        dark_options.sort_by_key(|(name, _)| name.to_lowercase());

        let settings = settings::snapshot();
        let saved_light = settings
            .light_theme_name
            .filter(|name| {
                registry
                    .themes()
                    .get(name.as_str())
                    .is_some_and(|theme| theme.mode == gpui_kit::component::ThemeMode::Light)
            })
            .unwrap_or_else(|| {
                gpui_kit::component::Theme::global(cx)
                    .light_theme
                    .name
                    .to_string()
            });
        let saved_dark = settings
            .dark_theme_name
            .filter(|name| {
                registry
                    .themes()
                    .get(name.as_str())
                    .is_some_and(|theme| theme.mode == gpui_kit::component::ThemeMode::Dark)
            })
            .unwrap_or_else(|| {
                gpui_kit::component::Theme::global(cx)
                    .dark_theme
                    .name
                    .to_string()
            });
        let font_size_multiplier = settings.font_scale;

        cx.set_global(ThemeOptions {
            light_theme_name: saved_light.into(),
            dark_theme_name: saved_dark.into(),
            light_options,
            dark_options,
            font_size_multiplier,
            theme_preference: settings.theme_preference.name().into(),
        });
    }

    fn theme_mode_item() -> SettingItem {
        let mode_options: Vec<(SharedString, SharedString)> = vec![
            ("system".into(), "System".into()),
            ("light".into(), "Light".into()),
            ("dark".into(), "Dark".into()),
        ];
        SettingItem::new(
            "Mode",
            SettingField::scrollable_dropdown(
                mode_options,
                |cx| cx.global::<ThemeOptions>().theme_preference.clone(),
                |val: SharedString, cx| Self::apply_theme_preference(&val, cx),
            ),
        )
        .description("Theme mode to be used.")
    }

    fn apply_theme_preference(val: &SharedString, cx: &mut App) {
        let Some(preference) = ThemePreference::from_name(val.as_str()) else {
            return;
        };
        if let Err(error) = settings::set_theme_preference(preference) {
            notifications::push_window_notification(
                cx,
                notifications::settings_save_failed("theme mode", &error),
            );
        }
        preferences::apply_theme_preference(preference, cx);
    }

    pub(super) fn theme_group(cx: &Context<DatalithView>) -> SettingGroup {
        let light_options = cx.global::<ThemeOptions>().light_options.clone();
        let dark_options = cx.global::<ThemeOptions>().dark_options.clone();

        SettingGroup::new().title("Theme").items(vec![
            Self::theme_mode_item(),
            SettingItem::new(
                "Light Theme",
                SettingField::scrollable_dropdown(
                    light_options,
                    |cx| cx.global::<ThemeOptions>().light_theme_name.clone(),
                    |val: SharedString, cx| {
                        cx.global_mut::<ThemeOptions>().light_theme_name = val.clone();
                        let registry = gpui_kit::component::ThemeRegistry::global(cx);
                        if let Some(theme_config) = registry
                            .themes()
                            .get(val.as_str())
                            .filter(|theme| theme.mode == gpui_kit::component::ThemeMode::Light)
                        {
                            gpui_kit::component::Theme::global_mut(cx).light_theme =
                                theme_config.clone();
                            let current_mode = gpui_kit::component::Theme::global(cx).mode;
                            gpui_kit::component::Theme::change(current_mode, None, cx);
                            gpui_kit::component::Theme::global_mut(cx).mode = current_mode;
                            if let Err(error) = settings::select_theme(ThemeKind::Light, &val) {
                                notifications::push_window_notification(
                                    cx,
                                    notifications::settings_save_failed("theme", &error),
                                );
                            }
                        }
                        cx.refresh_windows();
                    },
                ),
            )
            .description("Theme used in light mode."),
            SettingItem::new(
                "Dark Theme",
                SettingField::scrollable_dropdown(
                    dark_options,
                    |cx| cx.global::<ThemeOptions>().dark_theme_name.clone(),
                    |val: SharedString, cx| {
                        cx.global_mut::<ThemeOptions>().dark_theme_name = val.clone();
                        let registry = gpui_kit::component::ThemeRegistry::global(cx);
                        if let Some(theme_config) = registry
                            .themes()
                            .get(val.as_str())
                            .filter(|theme| theme.mode == gpui_kit::component::ThemeMode::Dark)
                        {
                            gpui_kit::component::Theme::global_mut(cx).dark_theme =
                                theme_config.clone();
                            let current_mode = gpui_kit::component::Theme::global(cx).mode;
                            gpui_kit::component::Theme::change(current_mode, None, cx);
                            gpui_kit::component::Theme::global_mut(cx).mode = current_mode;
                            if let Err(error) = settings::select_theme(ThemeKind::Dark, &val) {
                                notifications::push_window_notification(
                                    cx,
                                    notifications::settings_save_failed("theme", &error),
                                );
                            }
                        }
                        cx.refresh_windows();
                    },
                ),
            )
            .description("Theme used in dark mode."),
        ])
    }

    pub(super) fn display_group(font_size_slider_state: &Entity<SliderState>) -> SettingGroup {
        SettingGroup::new()
            .title("Display")
            .items(vec![SettingItem::render({
                let slider_state = font_size_slider_state.clone();
                move |_options, _window, cx| {
                    let value = slider_state.read(cx).value().start();
                    let label = format!("{value:.1}x");
                    let slider_state_clone = slider_state.clone();
                    h_flex()
                        .w_full()
                        .justify_between()
                        .gap_4()
                        .child(div().flex_1().child(Slider::new(&slider_state_clone)))
                        .child(label)
                        .into_any_element()
                }
            })])
    }
}
