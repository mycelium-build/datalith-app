use gpui::*;
use gpui_component::{
    ActiveTheme, IconName, Sizable, Size,
    button::{Button, ButtonVariants as _},
    h_flex,
    setting::{SettingField, SettingGroup, SettingItem, SettingPage, Settings},
    slider::{Slider, SliderState},
    v_flex,
};

use crate::config::{save_dark_theme_name, save_light_theme_name};

use super::DatalithView;

#[derive(Clone)]
pub(crate) struct ThemeOptions {
    pub light_theme_name: SharedString,
    pub dark_theme_name: SharedString,
    pub theme_options: Vec<(SharedString, SharedString)>,
    pub font_size_multiplier: f64,
}

impl Global for ThemeOptions {}

pub(crate) struct SettingsView {
    pub(crate) open: bool,
    focus_handle: FocusHandle,
    pub(crate) font_size_slider_state: Entity<SliderState>,
}

impl SettingsView {
    pub(crate) fn new(cx: &mut App) -> Self {
        let font_size_multiplier = crate::config::load_font_size_multiplier().unwrap_or(1.0);
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
        let font_size_multiplier = crate::config::load_font_size_multiplier().unwrap_or(1.0);

        cx.set_global(ThemeOptions {
            light_theme_name: saved_light,
            dark_theme_name: saved_dark,
            theme_options,
            font_size_multiplier,
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
                                                    theme_options.clone(),
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
                                                        if let Some(theme_config) =
                                                            registry.themes().get(val.as_str())
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
                                                        if let Some(theme_config) =
                                                            registry.themes().get(val.as_str())
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
                                                        }
                                                        cx.refresh_windows();
                                                        let _ = save_dark_theme_name(&val);
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
