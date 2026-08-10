use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Global, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, SharedString, StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::{
    ActiveTheme, IconName, Sizable, Size,
    button::{Button, ButtonVariants as _},
    h_flex,
    setting::{SettingField, SettingGroup, SettingItem, SettingPage, Settings},
    slider::{Slider, SliderState},
    v_flex,
};

use conv::{ConvUtil, UnwrapOrInf};

use crate::app::settings::{self, ThemeKind};
use crate::ui::monolith::monolith_mark;

use super::{DatalithView, notifications};

#[derive(Clone)]
pub struct ThemeOptions {
    pub(crate) light_theme_name: SharedString,
    pub(crate) dark_theme_name: SharedString,
    pub(crate) light_options: Vec<(SharedString, SharedString)>,
    pub(crate) dark_options: Vec<(SharedString, SharedString)>,
    pub(crate) font_size_multiplier: f64,
}

impl Global for ThemeOptions {}

pub struct SettingsView {
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
                .default_value(font_size_multiplier.approx_as::<f32>().unwrap_or_inf())
                .step(0.1)
        });
        Self {
            open: false,
            focus_handle: cx.focus_handle(),
            font_size_slider_state,
        }
    }

    pub(crate) const fn open(&mut self) {
        self.open = true;
    }

    pub(crate) const fn close(&mut self) {
        self.open = false;
    }

    pub(crate) fn init_theme_options(cx: &mut App) {
        let registry = gpui_component::ThemeRegistry::global(cx);
        let mut light_options: Vec<(SharedString, SharedString)> = registry
            .themes()
            .iter()
            .filter(|(_, theme)| theme.mode == gpui_component::ThemeMode::Light)
            .map(|(name, _)| (name.clone(), name.clone()))
            .collect();
        light_options.sort_by_key(|(name, _)| name.to_lowercase());
        let mut dark_options: Vec<(SharedString, SharedString)> = registry
            .themes()
            .iter()
            .filter(|(_, theme)| theme.mode == gpui_component::ThemeMode::Dark)
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
            light_options,
            dark_options,
            font_size_multiplier,
        });
    }

    pub(crate) fn render_overlay(&self, cx: &Context<DatalithView>) -> impl IntoElement {
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
                                vec![
                                        SettingPage::new("Appearance").default_open(true).groups(
                                            vec![
                                                Self::theme_group(cx),
                                                Self::display_group(&self.font_size_slider_state),
                                            ],
                                        ),
                                        SettingPage::new("Shortcuts")
                                            .groups(Self::shortcuts_groups()),
                                        SettingPage::new("About").groups(vec![Self::about_group()]),
                                    ],
                            )),
                    ),
            )
    }

    fn theme_group(cx: &Context<DatalithView>) -> SettingGroup {
        let light_options = cx.global::<ThemeOptions>().light_options.clone();
        let dark_options = cx.global::<ThemeOptions>().dark_options.clone();

        SettingGroup::new().title("Theme").items(vec![
            SettingItem::new(
                "Light Theme",
                SettingField::scrollable_dropdown(
                    light_options,
                    |cx| cx.global::<ThemeOptions>().light_theme_name.clone(),
                    |val: SharedString, cx| {
                        cx.global_mut::<ThemeOptions>().light_theme_name = val.clone();
                        let registry = gpui_component::ThemeRegistry::global(cx);
                        if let Some(theme_config) = registry
                            .themes()
                            .get(val.as_str())
                            .filter(|theme| theme.mode == gpui_component::ThemeMode::Light)
                        {
                            gpui_component::Theme::global_mut(cx).light_theme =
                                theme_config.clone();
                            let current_mode = gpui_component::Theme::global(cx).mode;
                            gpui_component::Theme::change(current_mode, None, cx);
                            gpui_component::Theme::global_mut(cx).mode = current_mode;
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
                        let registry = gpui_component::ThemeRegistry::global(cx);
                        if let Some(theme_config) = registry
                            .themes()
                            .get(val.as_str())
                            .filter(|theme| theme.mode == gpui_component::ThemeMode::Dark)
                        {
                            gpui_component::Theme::global_mut(cx).dark_theme = theme_config.clone();
                            let current_mode = gpui_component::Theme::global(cx).mode;
                            gpui_component::Theme::change(current_mode, None, cx);
                            gpui_component::Theme::global_mut(cx).mode = current_mode;
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

    fn display_group(font_size_slider_state: &Entity<SliderState>) -> SettingGroup {
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

    fn shortcuts_groups() -> Vec<SettingGroup> {
        struct Run {
            category: SharedString,
            description: SharedString,
            first_keys: SharedString,
            last_keys: SharedString,
        }

        let descriptions = crate::app::keymap::shortcut_descriptions();

        // Merge consecutive shortcuts that share a category and description into a range
        let mut runs: Vec<Run> = Vec::new();
        for (category, keys, description) in descriptions {
            let extends = runs.last().is_some_and(|run| {
                run.category.as_str() == category && run.description.as_str() == description
            });
            if extends && let Some(run) = runs.last_mut() {
                run.last_keys = keys.into();
            } else {
                runs.push(Run {
                    category: category.into(),
                    description: description.into(),
                    first_keys: keys.into(),
                    last_keys: keys.into(),
                });
            }
        }

        let merged: Vec<(SharedString, SharedString, SharedString)> = runs
            .into_iter()
            .map(|run| {
                let keys = if run.first_keys == run.last_keys {
                    run.first_keys
                } else {
                    format!("{} … {}", run.first_keys, run.last_keys).into()
                };
                (run.category, keys, run.description)
            })
            .collect();

        let mut groups: Vec<(SharedString, Vec<(SharedString, SharedString)>)> = Vec::new();
        for (category, keys, description) in merged {
            match groups.last_mut() {
                Some((cat, rows)) if cat.as_str() == category.as_str() => {
                    rows.push((keys, description));
                }
                _ => groups.push((category, vec![(keys, description)])),
            }
        }

        groups
            .into_iter()
            .map(|(category, rows)| {
                SettingGroup::new()
                    .title(category)
                    .items(vec![SettingItem::render(move |_options, _window, cx| {
                        v_flex()
                            .w_full()
                            .gap_1()
                            .children(rows.iter().map(|(keys, description)| {
                                h_flex()
                                    .w_full()
                                    .justify_between()
                                    .gap_4()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(description.clone()),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(cx.theme().muted)
                                            .text_sm()
                                            .child(keys.clone()),
                                    )
                                    .into_any_element()
                            }))
                            .into_any_element()
                    })])
            })
            .collect()
    }

    fn about_group() -> SettingGroup {
        let docs_vault = crate::app::docs::docs_vault_path()
            .to_string_lossy()
            .to_string();
        SettingGroup::new()
            .title("Datalith")
            .items(vec![SettingItem::render(move |_options, _window, cx| {
                v_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .child(monolith_mark(3.0, cx.theme().primary))
                    .child(div().font_weight(gpui::FontWeight::BOLD).child("Datalith"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("Version {}", env!("CARGO_PKG_VERSION"))),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("A fast, local-first knowledge workspace."),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("Docs Vault: {docs_vault}")),
                    )
                    .into_any_element()
            })])
    }
}
