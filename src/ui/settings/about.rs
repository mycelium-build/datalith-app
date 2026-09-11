//! About settings page: version, licensing, and legal links.

use gpui_kit::component::{
    ActiveTheme, Sizable as _,
    button::Button,
    setting::{SettingGroup, SettingItem},
    v_flex,
};
use gpui_kit::{App, IntoElement, ParentElement, Styled, div};

use super::{SETTINGS_PAGES, SettingsPage, SettingsView};
use crate::ui::monolith::monolith_mark;
use crate::ui::notifications;

const PRIVACY_POLICY_URL: &str = "https://mycelium-build.github.io/datalith/privacy/";
const TERMS_OF_SERVICE_URL: &str = "https://mycelium-build.github.io/datalith/terms/";

fn open_external_url(url: &str, cx: &mut App) {
    if let Err(error) = crate::app::system::open_url(url) {
        notifications::push_window_notification(cx, notifications::open_url_failed(url, &error));
    }
}

pub(super) fn about_page_index() -> usize {
    SETTINGS_PAGES
        .iter()
        .position(|page| *page == SettingsPage::About)
        .unwrap_or(0)
}

impl SettingsView {
    #[allow(clippy::too_many_lines)]
    pub(super) fn about_group() -> SettingGroup {
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
                    .child(
                        div()
                            .font_weight(gpui_kit::FontWeight::BOLD)
                            .child("Datalith"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("Version {}", crate::app::version::version())),
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
                                "Distributed binaries are conveyed under the MIT License; \
                                 third-party terms are in the license notices.",
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                "This program comes with ABSOLUTELY NO WARRANTY; \
                                 for details see the MIT License.",
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
                                    .on_click(|_, _, cx| {
                                        open_external_url(PRIVACY_POLICY_URL, cx);
                                    }),
                            )
                            .child(
                                Button::new("about-view-terms-of-service")
                                    .outline()
                                    .small()
                                    .label("Terms of service")
                                    .on_click(|_, _, cx| {
                                        open_external_url(TERMS_OF_SERVICE_URL, cx);
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
                                    .label("View release source")
                                    .on_click(|_, _, cx| {
                                        let url = crate::ui::licenses::corresponding_source_url();
                                        open_external_url(&url, cx);
                                    }),
                            ),
                    )
                    .into_any_element()
            }),
        ])
    }
}
