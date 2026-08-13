use gpui::{
    App, Context, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::{
    ActiveTheme, IconName, Sizable,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};

use super::DatalithView;

const MIT_LICENSE: &str = include_str!("../../LICENSE");
const GPL_LICENSE: &str = include_str!("../../LICENSE-GPL-3.0");
const LICENSING: &str = include_str!("../../LICENSING.md");
const THIRD_PARTY_NOTICES: &str = include_str!("../../THIRD-PARTY-NOTICES.md");

const RELEASE_REPO: &str = "https://github.com/mycelium-build/datalith";
const RELEASE_TAG: Option<&str> = option_env!("DATALITH_RELEASE_TAG");

/// The version-specific URL of the Corresponding Source archive for this build.
#[must_use]
pub fn corresponding_source_url() -> String {
    let tag = RELEASE_TAG.unwrap_or(concat!("v", env!("CARGO_PKG_VERSION")));
    format!("{RELEASE_REPO}/releases/tag/{tag}")
}

pub struct LicensesView {
    open: bool,
    focus_handle: FocusHandle,
}

impl LicensesView {
    pub fn new(cx: &App) -> Self {
        Self {
            open: false,
            focus_handle: cx.focus_handle(),
        }
    }

    pub const fn open(&mut self) {
        self.open = true;
    }

    pub const fn close(&mut self) {
        self.open = false;
    }

    pub(crate) const fn is_open(&self) -> bool {
        self.open
    }

    pub fn render_overlay(&self, cx: &Context<DatalithView>) -> impl IntoElement {
        let focus_handle = self.focus_handle.clone();
        div()
            .absolute()
            .inset_0()
            .bg(gpui::black().opacity(0.3))
            .flex()
            .items_center()
            .justify_center()
            .id("licenses-backdrop")
            .on_click(cx.listener(|view: &mut DatalithView, _, _, cx| {
                view.licenses.close();
                cx.notify();
            }))
            .child(
                div()
                    .w(px(760.))
                    .h(px(620.))
                    .bg(cx.theme().background)
                    .border(px(1.))
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .shadow_lg()
                    .id("licenses-panel")
                    .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()))
                    .track_focus(&focus_handle)
                    .on_key_down(cx.listener(
                        |view: &mut DatalithView, event: &KeyDownEvent, _, cx| {
                            if event.keystroke.key == "escape" {
                                view.licenses.close();
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
                                    .child(div().text_sm().child("Licenses"))
                                    .child(
                                        Button::new("close-licenses")
                                            .ghost()
                                            .small()
                                            .icon(IconName::Close)
                                            .on_click(cx.listener(|view, _, _, cx| {
                                                view.licenses.close();
                                                cx.notify();
                                            })),
                                    ),
                            )
                            .child(
                                div()
                                    .id("licenses-scroll")
                                    .flex_1()
                                    .overflow_y_scroll()
                                    .p_4()
                                    .children([
                                        section(
                                            "MIT License (original Datalith source)",
                                            MIT_LICENSE,
                                        ),
                                        section("GNU General Public License v3", GPL_LICENSE),
                                        section("Licensing overview", LICENSING),
                                        section("Third-party notices", THIRD_PARTY_NOTICES),
                                    ]),
                            ),
                    ),
            )
    }
}

fn section(title: &'static str, content: &'static str) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap_1()
        .mb_4()
        .child(
            div()
                .font_weight(gpui::FontWeight::BOLD)
                .text_sm()
                .child(title),
        )
        .child(div().font_family("monospace").text_xs().child(content))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_license_assets_are_non_empty() {
        assert!(!MIT_LICENSE.trim().is_empty());
        assert!(!GPL_LICENSE.trim().is_empty());
        assert!(!LICENSING.trim().is_empty());
        assert!(!THIRD_PARTY_NOTICES.trim().is_empty());
    }

    #[test]
    fn gpl_license_is_complete_gplv3() {
        assert!(GPL_LICENSE.contains("GNU GENERAL PUBLIC LICENSE"));
        assert!(GPL_LICENSE.contains("Version 3, 29 June 2007"));
    }

    #[test]
    fn corresponding_source_url_uses_package_version() {
        let url = corresponding_source_url();
        assert!(url.contains(RELEASE_TAG.unwrap_or(env!("CARGO_PKG_VERSION"))));
        assert!(url.starts_with(RELEASE_REPO));
        assert!(url.contains("/releases/tag/v"));
    }

    #[test]
    fn third_party_notices_cover_gpl_components() {
        assert!(THIRD_PARTY_NOTICES.contains("GPL-3.0-or-later"));
        assert!(THIRD_PARTY_NOTICES.contains("ztracing"));
    }

    #[test]
    fn licensing_overview_states_mit_and_gpl_scope() {
        assert!(LICENSING.contains("MIT"));
        assert!(LICENSING.contains("GPL-3.0-or-later"));
    }
}
