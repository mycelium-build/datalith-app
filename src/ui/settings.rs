use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Global, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, SharedString, StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::{
    ActiveTheme, IconName, Sizable, Size,
    button::{Button, ButtonVariants as _},
    h_flex,
    setting::{SelectIndex, SettingField, SettingGroup, SettingItem, SettingPage, Settings},
    slider::{Slider, SliderState},
    v_flex,
};

use conv::{ConvUtil, UnwrapOrInf};

use crate::app::preferences;
use crate::app::settings::{self, ThemeKind, ThemePreference};
use crate::ui::monolith::monolith_mark;

use super::{DatalithView, notifications};

const PRIVACY_POLICY_URL: &str = "https://mycelium-build.github.io/datalith/privacy/";
const TERMS_OF_SERVICE_URL: &str = "https://mycelium-build.github.io/datalith/terms/";

#[derive(Clone)]
pub struct ThemeOptions {
    pub(crate) light_theme_name: SharedString,
    pub(crate) dark_theme_name: SharedString,
    pub(crate) light_options: Vec<(SharedString, SharedString)>,
    pub(crate) dark_options: Vec<(SharedString, SharedString)>,
    pub(crate) font_size_multiplier: f64,
    pub(crate) theme_preference: SharedString,
}

impl Global for ThemeOptions {}

pub struct SettingsView {
    pub(crate) open: bool,
    focus_handle: FocusHandle,
    page_index: usize,
    pub(crate) font_size_slider_state: Entity<SliderState>,
}

/// The settings pages, in render order. Both the page builders and the
/// shortcuts page index derive from this list, so they cannot drift.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsPage {
    Appearance,
    Shortcuts,
    About,
}

const SETTINGS_PAGES: [SettingsPage; 3] = [
    SettingsPage::Appearance,
    SettingsPage::Shortcuts,
    SettingsPage::About,
];

impl SettingsPage {
    const fn title(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Shortcuts => "Shortcuts",
            Self::About => "About",
        }
    }
}

fn shortcuts_page_index() -> usize {
    SETTINGS_PAGES
        .iter()
        .position(|page| *page == SettingsPage::Shortcuts)
        .unwrap_or(0)
}

fn about_page_index() -> usize {
    SETTINGS_PAGES
        .iter()
        .position(|page| *page == SettingsPage::About)
        .unwrap_or(0)
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
            page_index: 0,
            font_size_slider_state,
        }
    }

    pub(crate) const fn open(&mut self) {
        self.open = true;
        self.page_index = 0;
    }

    pub(crate) fn open_shortcuts(&mut self) {
        self.open = true;
        self.page_index = shortcuts_page_index();
    }

    pub(crate) fn open_about(&mut self) {
        self.open = true;
        self.page_index = about_page_index();
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
            theme_preference: settings.theme_preference.name().into(),
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
                            .child(
                                Settings::new("app-settings")
                                    .with_size(Size::Small)
                                    .default_selected_index(SelectIndex {
                                        page_ix: self.page_index,
                                        group_ix: None,
                                    })
                                    .pages(self.settings_pages(cx)),
                            ),
                    ),
            )
    }

    fn settings_pages(&self, cx: &Context<DatalithView>) -> Vec<SettingPage> {
        SETTINGS_PAGES
            .iter()
            .map(|page| match page {
                SettingsPage::Appearance => SettingPage::new(page.title())
                    .default_open(true)
                    .groups(vec![
                        Self::theme_group(cx),
                        Self::display_group(&self.font_size_slider_state),
                    ]),
                SettingsPage::Shortcuts => {
                    SettingPage::new(page.title()).groups(Self::shortcuts_groups())
                }
                SettingsPage::About => {
                    SettingPage::new(page.title()).groups(vec![Self::about_group()])
                }
            })
            .collect()
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

    fn theme_group(cx: &Context<DatalithView>) -> SettingGroup {
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
        SettingGroup::new().title("Datalith").items(vec![
            SettingItem::render(move |_options, _window, cx| {
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
            }),
            SettingItem::render(move |_options, _window, cx| {
                v_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("Copyright (c) 2026 mycelium-build"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("Original Datalith source code: MIT License."),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                "Distributed binaries include GPL-3.0-or-later components \
                                 and are conveyed under GPL-3.0-or-later.",
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                "This program comes with ABSOLUTELY NO WARRANTY; \
                                 for details see the GNU GPL.",
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                Button::new("about-view-privacy-policy")
                                    .outline()
                                    .small()
                                    .label("Privacy policy")
                                    .on_click(|_, _, _cx| {
                                        let _ = crate::app::system::open_url(PRIVACY_POLICY_URL);
                                    }),
                            )
                            .child(
                                Button::new("about-view-terms-of-service")
                                    .outline()
                                    .small()
                                    .label("Terms of service")
                                    .on_click(|_, _, _cx| {
                                        let _ = crate::app::system::open_url(TERMS_OF_SERVICE_URL);
                                    }),
                            )
                            .child(
                                Button::new("about-view-licenses")
                                    .outline()
                                    .small()
                                    .label("View licenses")
                                    .on_click(|_, window, cx| {
                                        window.dispatch_action(
                                            Box::new(crate::app::actions::OpenLicenses),
                                            cx,
                                        );
                                    }),
                            )
                            .child(
                                Button::new("about-view-source")
                                    .outline()
                                    .small()
                                    .label("View corresponding source")
                                    .on_click(|_, _, _cx| {
                                        let url = crate::ui::licenses::corresponding_source_url();
                                        let _ = crate::app::system::open_url(&url);
                                    }),
                            ),
                    )
                    .into_any_element()
            }),
        ])
    }
}
