//! Render an isolated variant through Datalith's actual Markdown document view.
//!
//! The note renderer receives the selected variant explicitly; it never
//! changes the application's global theme or a user's open document.

use std::path::PathBuf;

use gpui_kit::component::input::EditorState;
use gpui_kit::component::{Theme, ThemeMode, h_flex, v_flex};
use gpui_kit::{App, Entity, IntoElement, ParentElement, SharedString, Styled, div};

use crate::ui::viewers::markdown::MarkdownViewer;

/// Effective installed (or fallback) fonts for one variant.
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

    #[must_use]
    pub fn render(self, note: &Entity<EditorState>, cx: &App) -> impl IntoElement {
        let content = note.read(cx).value().to_string();
        let viewer = MarkdownViewer::new(note.clone(), PathBuf::from("Field notes.md"));
        v_flex()
            .min_w_0()
            .min_h_0()
            .size_full()
            .bg(self.appearance.background)
            .text_color(self.appearance.foreground)
            .font_family(self.fonts.interface)
            .child(
                h_flex()
                    .gap_2()
                    .p_3()
                    .border_b_1()
                    .border_color(self.appearance.border)
                    .child(div().child("Note"))
                    .child(div().text_color(self.appearance.muted_foreground).child(
                        match self.appearance.mode {
                            ThemeMode::Light => "Light",
                            ThemeMode::Dark => "Dark",
                        },
                    )),
            )
            .child(div().flex_1().min_h_0().child(viewer.render_document(
                &content,
                None,
                &self.appearance,
                &[self.fonts.reading, self.fonts.headings, self.fonts.code],
                cx,
            )))
    }
}
