use gpui_kit::component::{
    ActiveTheme, IconName, Sizable, Size,
    button::{Button, ButtonVariants as _},
    h_flex,
    setting::{SelectIndex, SettingPage, Settings},
    setting::{SettingField, SettingGroup, SettingItem},
    slider::SliderState,
    v_flex,
};
use gpui_kit::{
    App, AppContext, Context, Entity, FocusHandle, Global, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, SharedString, StatefulInteractiveElement, Styled, div, px,
};

use conv::{ConvUtil, UnwrapOrInf};

use super::DatalithView;
use crate::app::settings;

pub const DOCS_URL: &str = "https://mycelium-build.github.io/datalith/docs/";

mod about;
mod appearance;
mod shortcuts;

use about::about_page_index;
use shortcuts::shortcuts_page_index;

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
    has_updater: bool,
    pub(crate) font_size_slider_state: Entity<SliderState>,
}

/// The settings pages, in render order. Both the page builders and the
/// shortcuts page index derive from this list, so they cannot drift.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SettingsPage {
    Appearance,
    Shortcuts,
    About,
}

pub(super) const SETTINGS_PAGES: [SettingsPage; 3] = [
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
            has_updater: crate::app::update::Updater::get(cx).is_some(),
            font_size_slider_state,
        }
    }

    pub(crate) const fn open(&mut self) {
        self.open = true;
        self.page_index = 0;
    }

    pub(crate) fn open_shortcuts(&mut self) {
        self.open = true;
        self.page_index = shortcuts_page_index().saturating_add(usize::from(self.has_updater));
    }

    pub(crate) fn open_about(&mut self) {
        self.open = true;
        self.page_index = about_page_index().saturating_add(usize::from(self.has_updater));
    }

    pub(crate) const fn close(&mut self) {
        self.open = false;
    }

    pub(crate) fn render_overlay(&self, cx: &Context<DatalithView>) -> impl IntoElement {
        div()
            .absolute()
            .inset_0()
            .bg(gpui_kit::black().opacity(0.3))
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
        let general = self.has_updater.then(|| {
            SettingPage::new("General").groups(vec![SettingGroup::new().title("Updates").items(
                vec![
                    SettingItem::new(
                        "Automatically update Datalith",
                        SettingField::switch(
                            |_| settings::snapshot().automatic_updates,
                            |enabled, cx| {
                                if let Err(error) = settings::set_automatic_updates(enabled) {
                                    crate::ui::notifications::push_window_notification(
                                        cx,
                                        crate::ui::notifications::settings_save_failed(
                                            "automatic updates",
                                            &error,
                                        ),
                                    );
                                }
                                cx.refresh_windows();
                            },
                        ),
                    )
                    .description("Periodically check for and download updates in the background."),
                ],
            )])
        });
        general
            .into_iter()
            .chain(SETTINGS_PAGES.iter().map(|page| {
                match page {
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
                }
            }))
            .collect()
    }
}
