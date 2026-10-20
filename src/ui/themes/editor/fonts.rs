//! The Fonts category: one specimen per role, at its real size and in the
//! edited variant's colors, above the role's font choice.
use gpui_kit::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, Theme, button::Button, h_flex, select::Select,
    v_flex,
};
use gpui_kit::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div,
};

use super::ThemeEditor;
use crate::app::{
    fonts::FontCatalog,
    settings::FontRole,
    themes::{ThemeLibrary, ThemeVariant},
};

fn specimen(role: FontRole, theme: &Theme, font: SharedString) -> AnyElement {
    match role {
        FontRole::Headings => div()
            .text_2xl()
            .font_weight(FontWeight::SEMIBOLD)
            .font_family(font)
            .child("A place for your ideas")
            .into_any_element(),
        FontRole::Reading => div()
            .text_base()
            .font_family(font)
            .child(
                "A quiet place to capture the details that matter. Follow the thread, \
                 then turn it into a plan.",
            )
            .into_any_element(),
        FontRole::Code => v_flex()
            .p_3()
            .rounded(theme.radius)
            .bg(theme.muted)
            .text_sm()
            .font_family(font)
            .child("let plan = build(\"Atlas\", 3);")
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .child("// Aa Bb Il1 O0 {} => 0123456789"),
            )
            .into_any_element(),
        FontRole::Interface => h_flex()
            .gap_3()
            .text_sm()
            .font_family(font)
            .child(
                div()
                    .px_3()
                    .py_1()
                    .rounded(theme.radius)
                    .bg(theme.secondary)
                    .text_color(theme.secondary_foreground)
                    .child("Open note"),
            )
            .child("Search notes")
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("Edited today"),
            )
            .into_any_element(),
    }
}

impl ThemeEditor {
    pub(super) fn render_fonts(&self, id: u64, cx: &Context<Self>) -> impl IntoElement {
        let document = cx
            .global::<ThemeLibrary>()
            .variant_by_id(id)
            .map(ThemeVariant::document);
        let catalog = cx.global::<FontCatalog>();
        v_flex()
            .w_full()
            .p_4()
            .gap_6()
            .children(
                FontRole::ALL
                    .into_iter()
                    .zip(&self.controls.fonts)
                    .map(|(role, state)| {
                        let unavailable = document
                            .and_then(|document| document.font(role))
                            .filter(|name| !catalog.contains(name));
                        v_flex()
                            .w_full()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(role.label()),
                            )
                            .children(self.preview.as_ref().map(|appearance| {
                                let theme = appearance.theme();
                                div()
                                    .p_4()
                                    .rounded(cx.theme().radius_lg)
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .bg(theme.background)
                                    .text_color(theme.foreground)
                                    .child(specimen(role, theme, appearance.font(role).clone()))
                            }))
                            .child(
                                Select::new(state)
                                    .small()
                                    .w_full()
                                    .search_placeholder("Search fonts")
                                    .accessibility_label(role.label()),
                            )
                            .children(unavailable.map(|font| {
                                h_flex()
                                    .gap_2()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(
                                        Icon::new(IconName::TriangleAlert)
                                            .small()
                                            .text_color(cx.theme().warning),
                                    )
                                    .child(format!(
                                        "{font} is unavailable; samples use the fallback font."
                                    ))
                            }))
                    }),
            )
            .child(
                h_flex().child(
                    Button::new(("apply-family-fonts", id))
                        .small()
                        .label("Apply fonts to all variants")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.apply_fonts_to_all(id, window, cx);
                        })),
                ),
            )
    }
}
