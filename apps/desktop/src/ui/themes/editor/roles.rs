//! The Colors category: base swatches, then one card per core group with a
//! specimen strip drawn from the edited variant's resolved theme.
use gpui_kit::component::{
    ActiveTheme as _, Icon, IconName, Selectable as _, Sizable as _, Theme, ThemeMode,
    button::{Button, ButtonGroup},
    color_picker::ColorPicker,
    h_flex,
    tooltip::Tooltip,
    try_parse_color, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, Context, Div, FontWeight, Hsla, InteractiveElement as _, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement as _, Styled, div,
};

use crate::app::themes::ThemeLibrary;

use super::{
    ThemeEditor, colors,
    render::{ESSENTIAL_COLORS, EssentialColor},
};

/// Samples drawn for one core color group.
type Specimens = fn(&Theme) -> Vec<Div>;

const ROLE_GROUPS: [(&str, Specimens); 3] = [
    (colors::PRIMARY_GROUP, primary_specimens),
    (colors::SECONDARY_GROUP, secondary_specimens),
    (colors::SURFACES_GROUP, surface_specimens),
];

/// The header and the Base card come before the group cards.
pub(super) const ROLE_ITEMS: usize = 2 + ROLE_GROUPS.len();

/// A flat sample chip in the variant's colors.
fn chip(theme: &Theme, background: Hsla, foreground: Hsla, text: &'static str) -> Div {
    div()
        .px_2()
        .py_0p5()
        .rounded(theme.radius)
        .border_1()
        .border_color(theme.transparent)
        .bg(background)
        .text_color(foreground)
        .text_xs()
        .child(text)
}

/// A sample with its caption below, both in the variant's colors.
fn specimen(theme: &Theme, caption: &'static str, sample: impl IntoElement) -> Div {
    v_flex()
        .gap_1()
        .items_start()
        .child(h_flex().min_h_6().child(sample))
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(caption),
        )
}

fn primary_specimens(theme: &Theme) -> Vec<Div> {
    let button = |background, caption| {
        specimen(
            theme,
            caption,
            chip(theme, background, theme.button_primary_foreground, "Save"),
        )
    };
    vec![
        button(theme.button_primary, "Rest"),
        button(theme.button_primary_hover, "Hover"),
        button(theme.button_primary_active, "Pressed"),
        specimen(
            theme,
            "Link",
            div()
                .text_xs()
                .text_color(theme.link)
                .underline()
                .child("Project Atlas"),
        ),
        specimen(
            theme,
            "Selection",
            h_flex()
                .text_xs()
                .child("Select ")
                .child(div().bg(theme.selection).child("this text")),
        ),
        specimen(
            theme,
            "Selected row",
            div()
                .px_2()
                .py_0p5()
                .rounded(theme.radius)
                .border_1()
                .border_color(theme.list_active_border)
                .bg(theme.list_active)
                .text_xs()
                .child("Field notes.md"),
        ),
    ]
}

fn secondary_specimens(theme: &Theme) -> Vec<Div> {
    let button = |background, caption| {
        specimen(
            theme,
            caption,
            chip(theme, background, theme.secondary_foreground, "Filter"),
        )
    };
    vec![
        button(theme.secondary, "Rest"),
        button(theme.secondary_hover, "Hover"),
        button(theme.secondary_active, "Open"),
        specimen(
            theme,
            "Menu",
            v_flex()
                .p_1()
                .rounded(theme.radius)
                .border_1()
                .border_color(theme.border)
                .bg(theme.popover)
                .text_color(theme.popover_foreground)
                .text_xs()
                .child(
                    div()
                        .px_2()
                        .rounded(theme.radius)
                        .bg(theme.accent)
                        .text_color(theme.accent_foreground)
                        .child("Rename…"),
                )
                .child(div().px_2().child("Export")),
        ),
        specimen(
            theme,
            "Hovered row",
            div()
                .px_2()
                .py_0p5()
                .rounded(theme.radius)
                .bg(theme.list_hover)
                .text_xs()
                .child("Overview.base"),
        ),
        specimen(
            theme,
            "Scrollbar",
            div().w_1p5().h_8().rounded_full().bg(theme.scrollbar_thumb),
        ),
    ]
}

fn surface_specimens(theme: &Theme) -> Vec<Div> {
    let input = |border, caption| {
        specimen(
            theme,
            caption,
            div()
                .w_24()
                .px_2()
                .py_0p5()
                .rounded(theme.radius)
                .border_1()
                .border_color(border)
                .bg(theme.background)
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("Search…"),
        )
    };
    vec![
        specimen(
            theme,
            "Muted text",
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("Edited today"),
        ),
        input(theme.input, "Input"),
        input(theme.ring, "Focused"),
        specimen(
            theme,
            "List header",
            v_flex()
                .w_24()
                .rounded(theme.radius)
                .border_1()
                .border_color(theme.border)
                .text_xs()
                .child(
                    div()
                        .px_2()
                        .bg(theme.list_head)
                        .font_weight(FontWeight::MEDIUM)
                        .child("Name"),
                )
                .child(div().px_2().bg(theme.colors.list).child("Atlas")),
        ),
    ]
}

impl ThemeEditor {
    fn render_base_tile(
        &self,
        id: u64,
        (token, label, description, _): &'static EssentialColor,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(row) = self.controls.colors.get(*token) else {
            return div().into_any_element();
        };
        let (token, label, description): (&'static str, &'static str, &'static str) =
            (token, label, description);
        let value: SharedString = if row.display.is_empty() {
            "Not set".into()
        } else {
            row.display.clone().into()
        };
        let swatch = self.render_base_swatch(id, token, label, &value, cx);
        v_flex()
            .id(format!("theme-token-{id}-{token}"))
            .min_w_0()
            .gap_1p5()
            .child(swatch)
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        v_flex()
                            .id(format!("theme-token-name-{id}-{token}"))
                            .flex_1()
                            .min_w_0()
                            .tooltip(move |window, cx| Tooltip::new(description).build(window, cx))
                            .child(div().truncate().text_sm().child(label))
                            .child(
                                div()
                                    .truncate()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(value),
                            ),
                    )
                    .child(Self::render_color_reset(id, token, label, row, cx).xsmall()),
            )
            .into_any_element()
    }

    fn render_base_swatch(
        &self,
        id: u64,
        token: &'static str,
        label: &'static str,
        value: &SharedString,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(row) = self.controls.colors.get(token) else {
            return div().into_any_element();
        };
        if let Some(controls) = row
            .controls
            .as_ref()
            .filter(|_| self.is_active_color(token))
        {
            return h_flex()
                .h_10()
                .px_2()
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(cx.theme().ring)
                .child(
                    ColorPicker::new(&controls.picker)
                        .small()
                        .label(value.clone())
                        .accessibility_label(format!("Pick {label} color")),
                )
                .into_any_element();
        }
        Button::new(format!("base-color-{id}-{token}"))
            .outline()
            .w_full()
            .h_10()
            .p_0p5()
            .tooltip(format!("Choose {label} color…"))
            .accessibility_label(format!("Edit {label} color, {value}"))
            .child(
                div()
                    .size_full()
                    .rounded(cx.theme().radius)
                    .when_some(try_parse_color(&row.display).ok(), |fill, color| {
                        fill.bg(color)
                    }),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.edit_color(id, token, window, cx);
            }))
            .into_any_element()
    }

    fn render_role_card(
        title: &'static str,
        header: impl IntoElement,
        body: impl IntoElement,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .id(SharedString::from(format!("theme-color-role-{title}")))
            .w_full()
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .px_4()
                    .py_2()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(title),
            )
            .child(header)
            .child(body)
    }

    /// Light or Dark, as a two-option segmented toggle.
    fn render_variant_mode(id: u64, cx: &Context<Self>) -> impl IntoElement {
        let dark = cx
            .global::<ThemeLibrary>()
            .variant_by_id(id)
            .is_some_and(|variant| variant.mode().is_dark());
        h_flex()
            .gap_3()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_sm().child("Appearance"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("Whether this variant is used in light or dark mode"),
                    ),
            )
            .child(
                ButtonGroup::new(("variant-mode", id))
                    .outline()
                    .small()
                    .child(
                        Button::new(("variant-light", id))
                            .icon(Icon::new(IconName::Sun))
                            .label("Light")
                            .selected(!dark),
                    )
                    .child(
                        Button::new(("variant-dark", id))
                            .icon(Icon::new(IconName::Moon))
                            .label("Dark")
                            .selected(dark),
                    )
                    .on_click(cx.listener(move |this, clicked: &Vec<usize>, window, cx| {
                        let mode = if clicked.first() == Some(&1) {
                            ThemeMode::Dark
                        } else {
                            ThemeMode::Light
                        };
                        this.change_variant_mode(id, mode, window, cx);
                    })),
            )
    }

    fn render_role_group(
        &self,
        id: u64,
        (group, specimens): (&'static str, Specimens),
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let strip = self.preview.as_ref().map(|appearance| {
            let theme = appearance.theme();
            h_flex()
                .px_4()
                .py_3()
                .gap_4()
                .flex_wrap()
                .items_start()
                .border_y_1()
                .border_color(cx.theme().border)
                .bg(theme.background)
                .text_color(theme.foreground)
                .children(specimens(theme))
        });
        Self::render_role_card(
            group,
            div().children(strip),
            v_flex().py_1().children(
                in_group(group)
                    .filter(|(token, ..)| self.controls.colors.contains_key(*token))
                    .map(|(token, ..)| self.render_color_row(id, token, cx)),
            ),
            cx,
        )
    }

    /// One item of the Colors list: the header, the Base card, then one
    /// card per core group. Only items in view are rendered.
    pub(super) fn render_role_item(&self, id: u64, ix: usize, cx: &Context<Self>) -> AnyElement {
        let item = match ix {
            0 => v_flex()
                .gap_4()
                .child(Self::render_variant_mode(id, cx))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(
                            "Base colors and common overrides. Component-specific colors in \
                             All colors take priority.",
                        ),
                )
                .into_any_element(),
            1 => Self::render_role_card(
                "Base",
                div(),
                div().px_4().pb_4().grid().grid_cols(3).gap_3().children(
                    in_group(colors::CORE_GROUP).map(|color| self.render_base_tile(id, color, cx)),
                ),
                cx,
            )
            .into_any_element(),
            _ => ix
                .checked_sub(2)
                .and_then(|group| ROLE_GROUPS.get(group))
                .map_or_else(
                    || div().into_any_element(),
                    |group| self.render_role_group(id, *group, cx).into_any_element(),
                ),
        };
        div()
            .w_full()
            .px_4()
            .pt_4()
            .when(ix.checked_add(1) == Some(ROLE_ITEMS), Styled::pb_4)
            .child(item)
            .into_any_element()
    }
}

fn in_group(group: &'static str) -> impl Iterator<Item = &'static EssentialColor> {
    ESSENTIAL_COLORS
        .iter()
        .filter(move |(.., essential_group)| *essential_group == group)
}
