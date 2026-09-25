//! Read-only, editor-local samples of a resolved theme variant.
//!
//! The caller resolves the selected variant into a complete `Theme` and four
//! installed (or fallback) font families. This view never reads or changes the
//! global theme: a selected non-current variant cannot leak into another view.

use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{Theme, ThemeMode, h_flex, v_flex};
use gpui_kit::{FontWeight, IntoElement, ParentElement, SharedString, Styled, StyledText, div};

/// Effective font families for each preview role, after availability fallback.
#[derive(Clone)]
pub struct PreviewFonts {
    interface: SharedString,
    reading: SharedString,
    headings: SharedString,
    code: SharedString,
}

impl PreviewFonts {
    #[must_use]
    pub const fn new(
        interface: SharedString,
        reading: SharedString,
        headings: SharedString,
        code: SharedString,
    ) -> Self {
        Self {
            interface,
            reading,
            headings,
            code,
        }
    }
}

/// Native note, base, and todo.txt samples for one explicitly resolved variant.
///
/// Place this inside the editor's preview region. Its container owns its vertical
/// scroll; the editor owns variant selection and the surrounding layout. Pass a
/// *resolved* `Theme` (colors, mode and highlight theme from the same variant),
/// not `cx.theme()` from the app's active appearance.
pub struct ThemePreview {
    appearance: Theme,
    fonts: PreviewFonts,
}

impl ThemePreview {
    #[must_use]
    pub fn new(appearance: &Theme, fonts: PreviewFonts) -> Self {
        Self {
            appearance: appearance.clone(),
            fonts,
        }
    }

    /// Compose the three samples; call from the editor's ordinary render path.
    #[must_use]
    pub fn render(self) -> impl IntoElement {
        let theme = &self.appearance;
        v_flex()
            .min_w_0()
            .min_h_0()
            .size_full()
            .overflow_y_scrollbar()
            .bg(theme.background)
            .text_color(theme.foreground)
            .font_family(self.fonts.interface.clone())
            .child(
                v_flex()
                    .w_full()
                    .min_w_0()
                    .p_4()
                    .gap_4()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child("Preview"),
                            )
                            .child(div().text_sm().text_color(theme.primary).child(
                                match theme.mode {
                                    ThemeMode::Light => "Light",
                                    ThemeMode::Dark => "Dark",
                                },
                            )),
                    )
                    .child(sample(theme, "Note", self.note()))
                    .child(sample(theme, "Base", self.base()))
                    .child(sample(theme, "todo.txt", self.todo())),
            )
    }

    fn note(&self) -> impl IntoElement {
        let theme = &self.appearance;
        v_flex()
            .gap_3()
            .min_w_0()
            .p_4()
            .bg(theme.background)
            .text_color(theme.foreground)
            .font_family(self.fonts.reading.clone())
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .font_family(self.fonts.headings.clone())
                    .text_color(syntax_color(theme, "title"))
                    .child("Field notes"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("A small space for ideas · Today"),
            )
            .child(
                div()
                    .whitespace_normal()
                    .child("A quiet place to capture the details that matter. Follow the thread, then turn it into a plan."),
            )
            .child(
                h_flex()
                    .items_start()
                    .min_w_0()
                    .pl_3()
                    .border_l_2()
                    .border_color(theme.primary)
                    .text_color(theme.muted_foreground)
                    .child("Make room for the next idea."),
            )
            .child(
                h_flex()
                    .gap_2()
                    .min_w_0()
                    .flex_wrap()
                    .child(div().text_color(theme.primary).child("•"))
                    .child(div().child("Link the draft to"))
                    .child(
                        div()
                            .text_color(syntax_color(theme, "link_text"))
                            .child("Project Atlas"),
                    ),
            )
            .child(
                div()
                    .min_w_0()
                    .rounded(theme.radius)
                    .bg(theme.highlight_theme.style.editor_background.unwrap_or(theme.muted))
                    .font_family(self.fonts.code.clone())
                    .text_sm()
                    .text_color(theme.highlight_theme.style.editor_foreground.unwrap_or(theme.foreground))
                    .child(code_row(theme, "1", &[("// next step", "comment")], false))
                    .child(code_row(
                        theme,
                        "2",
                        &[
                            ("let ", "keyword"),
                            ("plan", "variable"),
                            (" = ", "operator"),
                            ("build", "function"),
                            ("(", "punctuation.bracket"),
                            ("\"Atlas\"", "string"),
                            (", ", "punctuation.delimiter"),
                            ("3", "number"),
                            (");", "punctuation"),
                        ],
                        true,
                    ))
                    .child(code_row(
                        theme,
                        "3",
                        &[
                            ("const ", "keyword"),
                            ("READY", "constant"),
                            (": ", "punctuation.delimiter"),
                            ("bool", "type"),
                            (" = ", "operator"),
                            ("true", "boolean"),
                            (";", "punctuation"),
                        ],
                        false,
                    )),
            )
    }

    fn base(&self) -> impl IntoElement {
        let theme = &self.appearance;
        v_flex()
            .min_w_0()
            .bg(theme.table)
            .text_color(theme.foreground)
            .font_family(self.fonts.interface.clone())
            .child(
                h_flex()
                    .min_w_0()
                    .gap_2()
                    .p_3()
                    .bg(theme.tab_bar)
                    .border_b_1()
                    .border_color(theme.border)
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Projects"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("3 records"),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded(theme.radius)
                            .bg(theme.primary)
                            .text_color(theme.primary_foreground)
                            .text_sm()
                            .child("Active"),
                    ),
            )
            .child(base_row(theme, "Name", "Status", "Owner", true, false))
            .child(base_row(theme, "Atlas", "Active", "Mira", false, true))
            .child(base_row(theme, "Garden", "Planning", "Sam", false, false))
            .child(base_row(theme, "Archive", "Done", "Lee", false, false))
    }

    fn todo(&self) -> impl IntoElement {
        let theme = &self.appearance;
        v_flex()
            .min_w_0()
            .bg(theme.background)
            .text_color(theme.foreground)
            .font_family(self.fonts.interface.clone())
            .child(
                h_flex()
                    .p_3()
                    .gap_2()
                    .bg(theme.tab_bar)
                    .border_b_1()
                    .border_color(theme.border)
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Tasks"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("2 left"),
                    ),
            )
            .child(todo_row(
                theme,
                "☐",
                "(A)",
                "Outline the launch",
                "+atlas",
                "@desk",
                false,
            ))
            .child(todo_row(
                theme,
                "☐",
                "(B)",
                "Review the draft",
                "+atlas",
                "@work",
                true,
            ))
            .child(todo_row(
                theme,
                "☑",
                "",
                "Collect references",
                "+notes",
                "@desk",
                false,
            ))
    }
}

fn sample(theme: &Theme, label: &'static str, content: impl IntoElement) -> impl IntoElement {
    v_flex()
        .min_w_0()
        .gap_2()
        .child(
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(label),
        )
        .child(
            div()
                .min_w_0()
                .border_1()
                .border_color(theme.border)
                .rounded(theme.radius)
                .overflow_hidden()
                .child(content),
        )
}

fn base_row(
    theme: &Theme,
    name: &'static str,
    status: &'static str,
    owner: &'static str,
    header: bool,
    selected: bool,
) -> impl IntoElement {
    let background = if header {
        theme.table_head
    } else if selected {
        theme.table_active
    } else {
        theme.table
    };
    let foreground = if header {
        theme.table_head_foreground
    } else {
        theme.foreground
    };
    h_flex()
        .min_w_0()
        .items_start()
        .gap_2()
        .px_3()
        .py_2()
        .bg(background)
        .text_color(foreground)
        .border_b_1()
        .border_color(theme.table_row_border)
        .text_sm()
        .child(div().flex_1().min_w_0().child(name))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_color(if header { foreground } else { theme.primary })
                .child(status),
        )
        .child(div().flex_1().min_w_0().child(owner))
}

fn todo_row(
    theme: &Theme,
    check: &'static str,
    priority: &'static str,
    description: &'static str,
    project: &'static str,
    context: &'static str,
    selected: bool,
) -> impl IntoElement {
    h_flex()
        .min_w_0()
        .gap_2()
        .px_3()
        .py_2()
        .flex_wrap()
        .bg(if selected {
            theme.accent
        } else {
            theme.background
        })
        .text_color(if selected {
            theme.accent_foreground
        } else {
            theme.foreground
        })
        .border_b_1()
        .border_color(theme.border)
        .text_sm()
        .child(div().text_color(theme.primary).child(check))
        .child(div().text_color(theme.warning).child(priority))
        .child(div().child(description))
        .child(div().text_color(theme.info).child(project))
        .child(div().text_color(theme.success).child(context))
}

fn syntax_color(theme: &Theme, token: &str) -> gpui_kit::Hsla {
    theme
        .highlight_theme
        .style
        .syntax
        .style(token)
        .and_then(|style| style.color)
        .unwrap_or(theme.foreground)
}

fn code_row(
    theme: &Theme,
    number: &'static str,
    tokens: &[(&str, &str)],
    active: bool,
) -> impl IntoElement {
    let style = &theme.highlight_theme.style;
    let editor_background = style.editor_background.unwrap_or(theme.muted);
    h_flex()
        .items_start()
        .min_w_0()
        .bg(if active {
            style.editor_active_line.unwrap_or(editor_background)
        } else {
            editor_background
        })
        .child(
            div()
                .px_2()
                .py_1()
                .bg(style.editor_gutter_background.unwrap_or(editor_background))
                .text_color(if active {
                    style
                        .editor_active_line_number
                        .or(style.editor_line_number)
                        .unwrap_or(theme.muted_foreground)
                } else {
                    style.editor_line_number.unwrap_or(theme.muted_foreground)
                })
                .child(number),
        )
        .child(
            div()
                .min_w_0()
                .px_2()
                .py_1()
                .whitespace_normal()
                .child(syntax_line(theme, tokens)),
        )
}

fn syntax_line(theme: &Theme, tokens: &[(&str, &str)]) -> StyledText {
    let mut text = String::new();
    let mut highlights = Vec::with_capacity(tokens.len());
    for &(part, token) in tokens {
        let start = text.len();
        text.push_str(part);
        let mut style = theme
            .highlight_theme
            .style
            .syntax
            .style(token)
            .unwrap_or_default();
        if style.color.is_none() {
            style.color = Some(
                theme
                    .highlight_theme
                    .style
                    .editor_foreground
                    .unwrap_or(theme.foreground),
            );
        }
        highlights.push((start..text.len(), style));
    }
    StyledText::new(SharedString::from(text)).with_highlights(highlights)
}
