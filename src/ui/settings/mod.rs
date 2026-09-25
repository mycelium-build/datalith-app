use gpui_kit::base::FocusTrapElement as _;
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
    KeyDownEvent, ParentElement, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
    rems,
};

use conv::{ConvUtil, UnwrapOrInf};

use super::DatalithView;
use crate::app::settings;

pub const DOCS_URL: &str = "https://mycelium-build.github.io/datalith/docs/";

mod about;
mod appearance;
mod server;
#[cfg(test)]
mod tests;
pub mod theme;

use about::about_page_index;

#[derive(Clone)]
pub struct ThemeOptions {
    pub(crate) light_theme_name: SharedString,
    pub(crate) dark_theme_name: SharedString,
    pub(crate) font_size_multiplier: f64,
    pub(crate) theme_preference: SharedString,
}

impl Global for ThemeOptions {}

pub struct SettingsView {
    pub(crate) open: bool,
    focus_handle: FocusHandle,
    return_focus: Option<FocusHandle>,
    page_index: usize,
    navigation_revision: u64,
    has_updater: bool,
    pub(crate) font_size_slider_state: Entity<SliderState>,
    theme_page: Entity<theme::ThemePage>,
}

/// The settings pages, in render order; shortcuts have their own surface.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SettingsPage {
    Appearance,
    Theme,
    Server,
    About,
}

pub(super) const SETTINGS_PAGES: [SettingsPage; 4] = [
    SettingsPage::Appearance,
    SettingsPage::Theme,
    SettingsPage::Server,
    SettingsPage::About,
];

impl SettingsPage {
    const fn title(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Theme => "Theme",
            Self::Server => "Local Server",
            Self::About => "About",
        }
    }
}

impl SettingsView {
    pub(crate) fn new(window: &mut Window, cx: &mut App) -> Self {
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
            return_focus: None,
            page_index: 0,
            navigation_revision: 0,
            has_updater: crate::app::update::Updater::get(cx).is_some(),
            font_size_slider_state,
            theme_page: cx.new(|cx| theme::ThemePage::new(window, cx)),
        }
    }

    pub(crate) const fn open(&mut self) {
        self.open = true;
        self.page_index = 0;
        self.navigation_revision = self.navigation_revision.saturating_add(1);
    }

    pub(crate) const fn open_theme(&mut self) {
        self.open = true;
        self.page_index = if self.has_updater { 2 } else { 1 };
        self.navigation_revision = self.navigation_revision.saturating_add(1);
    }

    pub(crate) fn open_about(&mut self) {
        self.open = true;
        self.page_index = about_page_index().saturating_add(usize::from(self.has_updater)); // "General" page not displayed on dev channel
        self.navigation_revision = self.navigation_revision.saturating_add(1);
    }

    pub(crate) fn focus(&mut self, window: &mut Window, cx: &mut App) {
        self.return_focus = window.focused(cx);
        self.focus_handle.focus(window, cx);
    }

    fn dismiss(&mut self, window: &mut Window, cx: &mut App) {
        self.close();
        if let Some(focus) = self.return_focus.take() {
            focus.focus(window, cx);
        }
    }

    pub(crate) const fn close(&mut self) {
        self.open = false;
    }

    pub(crate) fn render_overlay(&self, cx: &Context<DatalithView>) -> impl IntoElement {
        div()
            .absolute()
            .inset_0()
            .bg(cx.theme().overlay)
            .flex()
            .items_center()
            .justify_center()
            .id("settings-backdrop")
            // Keep wheel events on the modal, including at its scroll boundaries.
            .occlude()
            .on_click(cx.listener(|view: &mut DatalithView, _, window, cx| {
                view.settings.dismiss(window, cx);
                cx.notify();
            }))
            .child(
                div()
                    .w(rems(60.))
                    .max_w_full()
                    .h(rems(42.))
                    .max_h_full()
                    .bg(cx.theme().background)
                    .border(px(1.))
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .shadow_lg()
                    .id("settings-panel")
                    .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()))
                    .on_key_down(cx.listener(
                        |view: &mut DatalithView, event: &KeyDownEvent, window, cx| {
                            if event.keystroke.key == "escape" {
                                view.settings.dismiss(window, cx);
                                cx.stop_propagation();
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
                                    .justify_between()
                                    .border_b(px(1.))
                                    .border_color(cx.theme().border)
                                    .child(div().text_sm().child("Settings"))
                                    .child(
                                        Button::new("close-settings")
                                            .ghost()
                                            .small()
                                            .icon(IconName::Close)
                                            .accessibility_label("Close preferences")
                                            .tooltip("Close")
                                            .on_click(cx.listener(|view, _, window, cx| {
                                                view.settings.dismiss(window, cx);
                                                cx.notify();
                                            })),
                                    ),
                            )
                            .child(
                                Settings::new(("app-settings", self.navigation_revision))
                                    .with_size(Size::Small)
                                    .default_selected_index(SelectIndex {
                                        page_ix: self.page_index,
                                        group_ix: None,
                                    })
                                    .pages(self.settings_pages(cx)),
                            ),
                    )
                    .focus_trap("preferences-focus", &self.focus_handle),
            )
    }

    fn settings_pages(&self, _cx: &Context<DatalithView>) -> Vec<SettingPage> {
        // In dev channel don't display update group
        // NOTE: need to move "General" page when add new content to it
        let general = self.has_updater.then(|| {
            SettingPage::new("General").groups(vec![SettingGroup::new().title("Updates").items(
                vec![
                    SettingItem::new(
                        format!(
                            "Automatically update {}",
                            crate::channel::Channel::current().product_name()
                        ),
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
                    .description(if crate::channel::Channel::current() == crate::channel::Channel::Preview {
                        "Periodically check for and download release candidates. Stable releases are not offered."
                    } else {
                        "Periodically check for and download updates in the background."
                    }),
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
                            Self::display_group(&self.font_size_slider_state),
                            Self::theme_navigation_group(),
                        ]),
                    SettingsPage::Theme => SettingPage::new(page.title())
                        .resettable(false)
                        .groups(vec![self.theme_page_group()]),
                    SettingsPage::Server => {
                        SettingPage::new(page.title()).groups(vec![Self::server_group()])
                    }
                    SettingsPage::About => {
                        SettingPage::new(page.title()).groups(vec![Self::about_group()])
                    }
                }
            }))
            .collect()
    }
}
