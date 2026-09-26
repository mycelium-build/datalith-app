//! Appearance settings page: display zoom and navigation to theme editing.

use gpui_kit::component::{
    ActiveTheme as _,
    button::Button,
    h_flex,
    setting::{SettingGroup, SettingItem},
    slider::{Slider, SliderState},
};
use gpui_kit::{App, Entity, IntoElement, ParentElement, Styled, div};

use super::{SettingsView, ThemeOptions};
use crate::app::settings;

impl SettingsView {
    pub(crate) fn init_theme_options(cx: &mut App) {
        let settings = settings::snapshot();
        let (saved_light, saved_dark) = cx
            .try_global::<crate::app::themes::ThemeLibrary>()
            .map_or_else(
                || ("Datalith Light".to_owned(), "Datalith Dark".to_owned()),
                |library| {
                    (
                        library.current(settings::ThemeKind::Light).to_owned(),
                        library.current(settings::ThemeKind::Dark).to_owned(),
                    )
                },
            );

        cx.set_global(ThemeOptions {
            light_theme_name: saved_light.into(),
            dark_theme_name: saved_dark.into(),
            font_size_multiplier: settings.font_scale,
            theme_preference: settings.theme_preference.name().into(),
        });
    }

    pub(super) fn theme_navigation_group() -> SettingGroup {
        SettingGroup::new()
            .title("Themes & Fonts")
            .items(vec![SettingItem::render(|_, _, cx| {
                gpui_kit::component::v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("Choose a theme or customize its colors and fonts."),
                    )
                    .child(
                        Button::new("manage-themes")
                            .label("Manage themes")
                            .on_click(|_, window, cx| {
                                if let Some(view) = cx
                                    .try_global::<crate::app::AppState>()
                                    .and_then(|state| state.view.clone())
                                {
                                    view.update(cx, |view, cx| {
                                        view.settings.open_theme(window, cx);
                                        // The navigation revision replaces the focused button.
                                        // Move focus to the retained modal before its old page drops.
                                        view.settings.focus_handle.focus(window, cx);
                                        cx.notify();
                                    });
                                }
                            }),
                    )
                    .into_any_element()
            })])
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
