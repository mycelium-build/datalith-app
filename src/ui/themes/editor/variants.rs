//! Variant navigation: miniature cards drawn from each variant's resolved
//! theme. Plain divs keep them cheap; only the full preview scopes the theme.
use gpui_kit::component::{
    ActiveTheme as _, Icon, IconName, Selectable as _, Sizable as _, Theme,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::Input,
    v_flex,
};
use std::rc::Rc;

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AnyView, App, Context, FontWeight, Hsla, InteractiveElement as _, IntoElement,
    ParentElement, Rems, Render, StatefulInteractiveElement as _, StyleRefinement, Styled, Window,
    div, relative, rems,
};

use super::{RenameTarget, ThemeEditor, render::variant_label};
use crate::{
    app::{
        settings::FontRole,
        themes::{ResolvedAppearance, ThemeFamily, ThemeVariant},
    },
    ui::icons::DatalithIcon,
};

const MINIATURE_HEIGHT: Rems = rems(7.);
pub(super) const VARIANT_RAIL_WIDTH: Rems = rems(15.);

/// A variant's miniature as its own view, so the editor can reuse its last
/// frame until the variant's appearance changes.
pub(super) struct VariantMiniature {
    appearance: Rc<ResolvedAppearance>,
}

impl VariantMiniature {
    pub(super) const fn new(appearance: Rc<ResolvedAppearance>) -> Self {
        Self { appearance }
    }

    pub(super) fn set_appearance(
        &mut self,
        appearance: Rc<ResolvedAppearance>,
        cx: &mut Context<Self>,
    ) {
        if !Rc::ptr_eq(&self.appearance, &appearance) {
            self.appearance = appearance;
            cx.notify();
        }
    }
}

impl Render for VariantMiniature {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        miniature(&self.appearance, cx)
    }
}

/// A small window drawn in the variant's own colors and fonts.
fn miniature(appearance: &ResolvedAppearance, cx: &App) -> impl IntoElement {
    let theme = appearance.theme();
    v_flex()
        .w_full()
        .h(MINIATURE_HEIGHT)
        .flex_none()
        .overflow_hidden()
        .rounded(cx.theme().radius)
        .border_1()
        .border_color(theme.border)
        .bg(theme.background)
        .text_color(theme.foreground)
        .child(
            div()
                .h_2p5()
                .flex_none()
                .bg(theme.title_bar)
                .border_b_1()
                .border_color(theme.title_bar_border),
        )
        .child(
            h_flex()
                .flex_1()
                .min_h_0()
                .items_stretch()
                .child(miniature_sidebar(theme))
                .child(miniature_note(appearance)),
        )
}

fn miniature_sidebar(theme: &Theme) -> impl IntoElement {
    let bar = || {
        div()
            .h_1()
            .w_full()
            .rounded_full()
            .bg(theme.sidebar_foreground.opacity(0.4))
    };
    v_flex()
        .w_1_4()
        .flex_none()
        .p_1()
        .gap_1()
        .bg(theme.sidebar)
        .border_r_1()
        .border_color(theme.sidebar_border)
        .child(bar())
        .child(
            div()
                .h_2()
                .w_full()
                .rounded_sm()
                .border_1()
                .border_color(theme.list_active_border)
                .bg(theme.list_active),
        )
        .child(bar())
        .child(bar())
}

fn miniature_note(appearance: &ResolvedAppearance) -> impl IntoElement {
    let theme = appearance.theme();
    let dot = |color: Hsla| div().size_1p5().flex_none().rounded_full().bg(color);
    v_flex()
        .flex_1()
        .min_w_0()
        .px_2()
        .py_1p5()
        .gap_1()
        .child(
            div()
                .text_sm()
                .line_height(relative(1.2))
                .truncate()
                .font_family(appearance.font(FontRole::Headings).clone())
                .font_weight(FontWeight::SEMIBOLD)
                .child("Heading"),
        )
        .child(
            div()
                .text_xs()
                .line_height(relative(1.2))
                .truncate()
                .font_family(appearance.font(FontRole::Reading).clone())
                .child("Reading text flows"),
        )
        .child(
            h_flex().child(
                div()
                    .px_1()
                    .rounded_sm()
                    .bg(theme.muted)
                    .text_xs()
                    .line_height(relative(1.3))
                    .font_family(appearance.font(FontRole::Code).clone())
                    .child("build()"),
            ),
        )
        .child(
            h_flex()
                .gap_1p5()
                .text_xs()
                .line_height(relative(1.3))
                .child(
                    div()
                        .px_1p5()
                        .rounded_sm()
                        .bg(theme.button_primary)
                        .text_color(theme.button_primary_foreground)
                        .child("Open"),
                )
                .child(div().text_color(theme.link).underline().child("Link"))
                .child(div().flex_1())
                .child(dot(theme.success))
                .child(dot(theme.warning))
                .child(dot(theme.danger)),
        )
}

impl ThemeEditor {
    /// Rename and delete, always visible beside the variant's name.
    fn render_variant_actions(id: u64, label: &str, cx: &Context<Self>) -> impl IntoElement {
        h_flex()
            .gap_0p5()
            .child(
                Button::new(("rename-variant", id))
                    .xsmall()
                    .ghost()
                    .icon(Icon::new(DatalithIcon::Pen))
                    .tooltip("Rename variant…")
                    .accessibility_label(format!("Rename {label}"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.start_variant_rename(id, window, cx);
                    })),
            )
            .child(
                Button::new(("remove-variant", id))
                    .xsmall()
                    .ghost()
                    .icon(IconName::Close)
                    .tooltip("Delete variant")
                    .accessibility_label(format!("Delete {label}"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.remove_variant(id, window, cx);
                    })),
            )
    }

    fn render_variant_card(
        &self,
        family: &ThemeFamily,
        variant: &ThemeVariant,
        cx: &Context<Self>,
    ) -> AnyElement {
        let id = variant.id();
        let mode = variant.mode();
        let label = variant_label(family, variant);
        let rename = self
            .rename
            .as_ref()
            .filter(|session| session.target == RenameTarget::Variant(id));
        let mode_label = if mode.is_dark() { "dark" } else { "light" };
        let accessibility = format!("{label}, {mode_label} variant");
        let mode_icon = Icon::new(if mode.is_dark() {
            IconName::Moon
        } else {
            IconName::Sun
        })
        .xsmall()
        .text_color(cx.theme().muted_foreground);
        // Controls on top of the card occlude it, so their clicks never also
        // select the variant.
        let name_row =
            h_flex()
                .w_full()
                .min_w_0()
                .gap_1p5()
                .child(mode_icon)
                .map(|row| match rename {
                    Some(session) => row.child(
                        div().flex_1().min_w_0().occlude().child(
                            Input::new(&session.input)
                                .id(format!("variant-name-input-{id}"))
                                .xsmall()
                                .w_full()
                                .aria_label("Variant name"),
                        ),
                    ),
                    None => row
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_sm()
                                .child(label.clone()),
                        )
                        .child(
                            div()
                                .flex_none()
                                .occlude()
                                .child(Self::render_variant_actions(id, &label, cx)),
                        ),
                });
        // The select button fills the card behind its content, which has no
        // pointer handlers of its own and so lets clicks reach the button.
        let select = Button::new(("select-variant", id))
            .ghost()
            .selected(id == self.edited)
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .accessibility_label(accessibility)
            .on_click(cx.listener(move |this, _, window, cx| this.select(id, window, cx)));
        div()
            .id(("theme-variant-card", id))
            .relative()
            .w_full()
            .child(select)
            .child(
                v_flex()
                    .w_full()
                    .min_w_0()
                    .p_1p5()
                    .gap_1p5()
                    .children(self.variant_miniatures.get(&id).map(|miniature| {
                        AnyView::from(miniature.clone()).cached(
                            StyleRefinement::default()
                                .w_full()
                                .h(MINIATURE_HEIGHT)
                                .flex_none(),
                        )
                    }))
                    .child(name_row),
            )
            .into_any_element()
    }

    fn render_add_variant(cx: &Context<Self>) -> Button {
        Button::new("add-variant")
            .small()
            .ghost()
            .icon(IconName::Plus)
            .label("Add variant…")
            .on_click(cx.listener(|this, _, window, cx| this.add_variant(window, cx)))
    }

    /// Wide layout: a vertical rail beside the controls.
    pub(super) fn render_variant_rail(
        &self,
        family: &ThemeFamily,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .id("theme-variant-navigation")
            .w(VARIANT_RAIL_WIDTH)
            .h_full()
            .flex_none()
            .min_h_0()
            .border_r_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .px_3()
                    .pt_3()
                    .pb_2()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("Variants"),
            )
            .child(
                v_flex()
                    .id("theme-variants-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_2()
                    .pb_2()
                    .gap_2()
                    .children(
                        family
                            .variants()
                            .iter()
                            .map(|variant| self.render_variant_card(family, variant, cx)),
                    )
                    .child(Self::render_add_variant(cx).w_full()),
            )
    }

    /// Narrow layout: the same cards in a horizontal strip above the controls.
    pub(super) fn render_variant_strip(
        &self,
        family: &ThemeFamily,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .id("theme-variant-navigation")
            .w_full()
            .flex_none()
            .items_start()
            .overflow_x_scroll()
            .p_2()
            .gap_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .children(family.variants().iter().map(|variant| {
                div()
                    .w(rems(13.))
                    .flex_none()
                    .child(self.render_variant_card(family, variant, cx))
            }))
            .child(Self::render_add_variant(cx).flex_none())
    }
}
