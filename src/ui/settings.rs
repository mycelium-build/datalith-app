use gpui::*;
use gpui_component::{
    ActiveTheme, IconName, Sizable, Size,
    button::{Button, ButtonVariants as _},
    h_flex,
    setting::{SettingField, SettingGroup, SettingItem, SettingPage, Settings},
    slider::{Slider, SliderState},
    v_flex,
};

use crate::app::settings::{self, ThemeKind};

use super::DatalithView;

#[derive(Clone)]
pub(crate) struct ThemeOptions {
    pub(crate) light_theme_name: SharedString,
    pub(crate) dark_theme_name: SharedString,
    pub(crate) light_theme_options: Vec<(SharedString, SharedString)>,
    pub(crate) dark_theme_options: Vec<(SharedString, SharedString)>,
    pub(crate) font_size_multiplier: f64,
}

impl Global for ThemeOptions {}

pub(crate) struct SettingsView {
    pub(crate) open: bool,
    focus_handle: FocusHandle,
    pub(crate) font_size_slider_state: Entity<SliderState>,
}

impl SettingsView {
    pub(crate) fn new(cx: &mut App) -> Self {
        let font_size_multiplier = settings::snapshot().font_scale;
        let font_size_slider_state = cx.new(|_| {
            SliderState::new()
                .min(0.5)
                .max(3.0)
                .default_value(font_size_multiplier as f32)
                .step(0.1)
        });
        Self {
            open: false,
            focus_handle: cx.focus_handle(),
            font_size_slider_state,
        }
    }

    pub(crate) fn open(&mut self) {
        self.open = true;
    }

    pub(crate) fn close(&mut self) {
        self.open = false;
    }

    pub(crate) fn init_theme_options(cx: &mut App) {
        let registry = gpui_component::ThemeRegistry::global(cx);
        let mut light_theme_options: Vec<(SharedString, SharedString)> = registry
            .themes()
            .iter()
            .filter(|(_, theme)| theme.mode == gpui_component::ThemeMode::Light)
            .map(|(name, _)| (name.clone(), name.clone()))
            .collect();
        light_theme_options.sort_by_key(|(name, _)| name.to_lowercase());
        let mut dark_theme_options: Vec<(SharedString, SharedString)> = registry
            .themes()
            .iter()
            .filter(|(_, theme)| theme.mode == gpui_component::ThemeMode::Dark)
            .map(|(name, _)| (name.clone(), name.clone()))
            .collect();
        dark_theme_options.sort_by_key(|(name, _)| name.to_lowercase());

        let settings = settings::snapshot();
        let saved_light = settings
            .light_theme_name
            .filter(|name| {
                registry
                    .themes()
                    .get(name.as_str())
                    .is_some_and(|theme| theme.mode == gpui_component::ThemeMode::Light)
            })
            .unwrap_or_else(|| {
                gpui_component::Theme::global(cx)
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
                    .is_some_and(|theme| theme.mode == gpui_component::ThemeMode::Dark)
            })
            .unwrap_or_else(|| {
                gpui_component::Theme::global(cx)
                    .dark_theme
                    .name
                    .to_string()
            });
        let font_size_multiplier = settings.font_scale;

        cx.set_global(ThemeOptions {
            light_theme_name: saved_light.into(),
            dark_theme_name: saved_dark.into(),
            light_theme_options,
            dark_theme_options,
            font_size_multiplier,
        });
    }

    pub(crate) fn render_overlay(&self, cx: &mut Context<DatalithView>) -> impl IntoElement {
        let light_theme_options = cx.global::<ThemeOptions>().light_theme_options.clone();
        let dark_theme_options = cx.global::<ThemeOptions>().dark_theme_options.clone();

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
                    .track_focus(&self.focus_handle)
                    .on_key_down(cx.listener(
                        |view: &mut DatalithView, event: &KeyDownEvent, _, cx| {
                            if event.keystroke.key == "escape" {
                                view.settings.close();
                                cx.notify();
                            }
                        },
                    ))
                    .child(
                        v_flex()
                            .size_full()
                            .overflow_hidden()
                            .child(
                                h_flex()
                                    .w_full()
                                    .px_2()
                                    .py_1()
                                    .justify_end()
                                    .border_b(px(1.))
                                    .border_color(cx.theme().border)
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
                            .child(Settings::new("app-settings").with_size(Size::Small).pages(
                                vec![SettingPage::new("Appearance").default_open(true).groups(
                                    vec![
                                        SettingGroup::new().title("Theme").items(vec![
                                            SettingItem::new(
                                                "Light Theme",
                                                SettingField::scrollable_dropdown(
                                                    light_theme_options,
                                                    |cx| {
                                                        cx.global::<ThemeOptions>()
                                                            .light_theme_name
                                                            .clone()
                                                    },
                                                    |val: SharedString, cx| {
                                                        cx.global_mut::<ThemeOptions>()
                                                            .light_theme_name = val.clone();
                                                        let registry =
                                                            gpui_component::ThemeRegistry::global(
                                                                cx,
                                                            );
                                                        if let Some(theme_config) = registry
                                                            .themes()
                                                            .get(val.as_str())
                                                            .filter(|theme| {
                                                                theme.mode
                                                                    == gpui_component::ThemeMode::Light
                                                            })
                                                        {
                                                            gpui_component::Theme::global_mut(cx)
                                                                .light_theme = theme_config.clone();
                                                            let current_mode =
                                                                gpui_component::Theme::global(cx)
                                                                    .mode;
                                                            gpui_component::Theme::change(
                                                                current_mode,
                                                                None,
                                                                cx,
                                                            );
                                                            gpui_component::Theme::global_mut(cx)
                                                                .mode = current_mode;
                                                            let _ = settings::select_theme(
                                                                ThemeKind::Light,
                                                                &val,
                                                            );
                                                        }
                                                        cx.refresh_windows();
                                                    },
                                                ),
                                            )
                                            .description("Theme used in light mode."),
                                            SettingItem::new(
                                                "Dark Theme",
                                                SettingField::scrollable_dropdown(
                                                    dark_theme_options,
                                                    |cx| {
                                                        cx.global::<ThemeOptions>()
                                                            .dark_theme_name
                                                            .clone()
                                                    },
                                                    |val: SharedString, cx| {
                                                        cx.global_mut::<ThemeOptions>()
                                                            .dark_theme_name = val.clone();
                                                        let registry =
                                                            gpui_component::ThemeRegistry::global(
                                                                cx,
                                                            );
                                                        if let Some(theme_config) = registry
                                                            .themes()
                                                            .get(val.as_str())
                                                            .filter(|theme| {
                                                                theme.mode
                                                                    == gpui_component::ThemeMode::Dark
                                                            })
                                                        {
                                                            gpui_component::Theme::global_mut(cx)
                                                                .dark_theme = theme_config.clone();
                                                            let current_mode =
                                                                gpui_component::Theme::global(cx)
                                                                    .mode;
                                                            gpui_component::Theme::change(
                                                                current_mode,
                                                                None,
                                                                cx,
                                                            );
                                                            gpui_component::Theme::global_mut(cx)
                                                                .mode = current_mode;
                                                            let _ = settings::select_theme(
                                                                ThemeKind::Dark,
                                                                &val,
                                                            );
                                                        }
                                                        cx.refresh_windows();
                                                    },
                                                ),
                                            )
                                            .description("Theme used in dark mode."),
                                        ]),
                                        SettingGroup::new().title("Display").items(vec![
                                            SettingItem::render({
                                                let slider_state =
                                                    self.font_size_slider_state.clone();
                                                move |_options, _window, cx| {
                                                    let value =
                                                        slider_state.read(cx).value().start();
                                                    let label = format!("{:.1}x", value);
                                                    let slider_state_clone = slider_state.clone();
                                                    h_flex()
                                                        .w_full()
                                                        .justify_between()
                                                        .gap_4()
                                                        .child(div().flex_1().child(Slider::new(
                                                            &slider_state_clone,
                                                        )))
                                                        .child(label)
                                                        .into_any_element()
                                                }
                                            }),
                                        ]),
                                    ],
                                )],
                            )),
                    ),
            )
    }
}
