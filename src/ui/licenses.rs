use std::sync::LazyLock;

use gpui::{
    App, Context, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    StatefulInteractiveElement, Styled, UniformListScrollHandle, div, px, uniform_list,
};
use gpui_component::{
    ActiveTheme, IconName, Sizable,
    button::{Button, ButtonVariants as _},
    h_flex,
    scroll::{Scrollbar, ScrollbarShow},
    v_flex,
};

use super::DatalithView;

const MIT_LICENSE: &str = include_str!("../../LICENSE");
const GPL_LICENSE: &str = include_str!("../../LICENSE-GPL-3.0");
const LICENSING: &str = include_str!("../../LICENSING.md");
const THIRD_PARTY_NOTICES: &str = include_str!("../../THIRD-PARTY-NOTICES.md");

const RELEASE_REPO: &str = "https://github.com/mycelium-build/datalith";
const RELEASE_TAG: Option<&str> = option_env!("DATALITH_RELEASE_TAG");

const ROW_HEIGHT: f32 = 20.0;

/// Approximate character width the licenses panel's content area can fit on one monospace line,
/// used to pre-wrap paragraphs once
/// instead of relying on GPUI to reflow the entire multi-megabyte notices text on every frame.
const WRAP_WIDTH: usize = 100; // chars

enum Row {
    Header(String),
    Line(String),
}

static ROWS: LazyLock<Vec<Row>> = LazyLock::new(|| {
    let sections = [
        ("MIT License (original Datalith source)", MIT_LICENSE),
        ("GNU General Public License v3", GPL_LICENSE),
        ("Licensing overview", LICENSING),
        ("Third-party notices", THIRD_PARTY_NOTICES),
    ];

    let mut rows = Vec::new();
    for (title, content) in sections {
        if !rows.is_empty() {
            rows.push(Row::Line(String::new()));
        }
        rows.push(Row::Header(title.to_string()));
        for line in content.lines() {
            rows.extend(wrap_line(line, WRAP_WIDTH).into_iter().map(Row::Line));
        }
    }
    rows
});

fn wrap_line(line: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in line.split_whitespace() {
        let fits_current = current.len().saturating_add(1).saturating_add(word.len()) <= width;
        if !current.is_empty() {
            if fits_current {
                current.push(' ');
            } else {
                lines.push(std::mem::take(&mut current));
            }
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[must_use]
pub fn corresponding_source_url() -> String {
    let tag = RELEASE_TAG.unwrap_or(concat!("v", env!("CARGO_PKG_VERSION")));
    format!("{RELEASE_REPO}/releases/tag/{tag}")
}

pub struct LicensesView {
    open: bool,
    focus_handle: FocusHandle,
    scroll_handle: UniformListScrollHandle,
}

impl LicensesView {
    pub fn new(cx: &App) -> Self {
        Self {
            open: false,
            focus_handle: cx.focus_handle(),
            scroll_handle: UniformListScrollHandle::new(),
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
                                    .relative()
                                    .flex_1()
                                    .child(
                                        uniform_list(
                                            "licenses-scroll",
                                            ROWS.len(),
                                            |range, _, _| {
                                                range
                                                    .filter_map(|ix| ROWS.get(ix).map(render_row))
                                                    .collect()
                                            },
                                        )
                                        .track_scroll(&self.scroll_handle)
                                        .size_full()
                                        .px_4()
                                        .py_2(),
                                    )
                                    .child(
                                        div().absolute().inset_0().child(
                                            Scrollbar::vertical(&self.scroll_handle)
                                                .scrollbar_show(ScrollbarShow::Always),
                                        ),
                                    ),
                            ),
                    ),
            )
    }
}

fn render_row(row: &Row) -> impl IntoElement {
    let (text, is_header) = match row {
        Row::Header(title) => (title.as_str(), true),
        Row::Line(line) => (line.as_str(), false),
    };

    let row = div()
        .h(px(ROW_HEIGHT))
        .flex()
        .items_center()
        .font_family("monospace")
        .text_xs()
        .child(text.to_string());

    if is_header {
        row.font_weight(gpui::FontWeight::BOLD).text_sm()
    } else {
        row
    }
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

    #[test]
    fn wrap_line_splits_long_lines_on_word_boundaries() {
        let wrapped = wrap_line("one two three four five", 10);
        assert!(wrapped.iter().all(|line| line.len() <= 10));
        assert_eq!(wrapped.join(" "), "one two three four five");
    }

    #[test]
    fn wrap_line_preserves_blank_lines() {
        assert_eq!(wrap_line("", 10), vec![String::new()]);
        assert_eq!(wrap_line("   ", 10), vec![String::new()]);
    }

    #[test]
    fn wrap_line_keeps_an_overlong_word_intact() {
        let word = "x".repeat(50);
        let wrapped = wrap_line(&word, 10);
        assert_eq!(wrapped, vec![word]);
    }

    #[test]
    fn rows_cover_every_section_with_a_header() {
        let header_count = ROWS
            .iter()
            .filter(|row| matches!(row, Row::Header(_)))
            .count();
        assert_eq!(header_count, 4);
        assert!(matches!(ROWS.first(), Some(Row::Header(_))));
        // Wrapping must never produce a row wider than the configured budget.
        for row in ROWS.iter() {
            let text = match row {
                Row::Header(text) | Row::Line(text) => text,
            };
            assert!(text.len() <= WRAP_WIDTH || !text.contains(' '));
        }
    }
}
