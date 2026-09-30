use super::{ColorOriginFilters, ColorRow, PropertyRow, RenameTarget, ThemeEditor, colors};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Selectable as _, Sizable as _, ThemeMode,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    color_picker::ColorPicker,
    h_flex,
    input::Input,
    resizable::{h_resizable, resizable_panel},
    scroll::ScrollableElement as _,
    select::Select,
    try_parse_color, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, Context, FontWeight, InteractiveElement as _, IntoElement, ParentElement, Rems,
    Render, StatefulInteractiveElement as _, Styled, Window, canvas, div, list, rems,
};

use crate::{
    app::{
        fonts::FontCatalog,
        settings::FontRole,
        themes::{SaveStatus, ThemeFamily, ThemeLibrary, ThemeVariant},
    },
    ui::icons::DatalithIcon,
};

// Base roles first, then common overrides used by the bundled themes and Datalith.
pub(super) const ESSENTIAL_COLORS: &[(&str, &str, &str)] = &[
    (
        "background",
        "Background",
        "The main workspace and reading surface",
    ),
    ("foreground", "Text", "Body text and interface labels"),
    (
        "muted.background",
        "Muted surface",
        "Subdued surfaces such as code blocks and note properties",
    ),
    ("border", "Borders", "Dividers between panels and controls"),
    ("primary.background", "Primary", "Primary action background"),
    (
        "secondary.background",
        "Secondary",
        "Secondary action background",
    ),
    (
        "primary.foreground",
        "Text on primary",
        "Text on primary actions and checkbox marks. Reset follows Text.",
    ),
    (
        "primary.hover.background",
        "Primary hover",
        "Hovered primary actions. Reset derives from Primary and Background.",
    ),
    (
        "primary.active.background",
        "Primary pressed",
        "Pressed primary actions. Reset darkens Primary.",
    ),
    (
        "link",
        "Link",
        "Links in notes, properties and Base. Reset follows Primary.",
    ),
    (
        "selection.background",
        "Text selection",
        "Selected text in fields and source editors. Reset follows Primary.",
    ),
    (
        "list.active.background",
        "Selected row",
        "Selected files, Todo tasks and choices. Reset derives from Primary and Background.",
    ),
    (
        "list.active.border",
        "Selected row border",
        "Selected choices and file context-menu outlines. Reset derives from Primary and Background.",
    ),
    (
        "secondary.foreground",
        "Text on secondary",
        "Toolbar buttons and supporting surfaces. Reset follows Text.",
    ),
    (
        "secondary.hover.background",
        "Secondary hover",
        "Custom title-bar button hover. Reset derives from Secondary and Background.",
    ),
    (
        "secondary.active.background",
        "Secondary selected",
        "Selected or open toolbar buttons. Reset darkens Secondary.",
    ),
    (
        "accent.background",
        "Accent",
        "Menus and general highlights. Reset follows Secondary.",
    ),
    (
        "accent.foreground",
        "Text on accent",
        "Text on highlighted menu items. Reset follows Text.",
    ),
    (
        "list.hover.background",
        "Hovered row",
        "Hovered files and choices. Reset follows Accent.",
    ),
    (
        "scrollbar.thumb.background",
        "Scrollbar thumb",
        "Scrollbar handles. Reset follows Accent.",
    ),
    (
        "muted.foreground",
        "Muted text",
        "Hints, metadata and completed tasks. Reset blends Muted surface and Text.",
    ),
    (
        "input.border",
        "Input border",
        "Text-field and select borders. Reset follows Borders.",
    ),
    (
        "ring",
        "Focus ring",
        "Focused text fields and controls. Reset follows the Blue palette color.",
    ),
    (
        "list.background",
        "List background",
        "List surface and default table background. Reset follows Background.",
    ),
    (
        "list.head.background",
        "List / table header",
        "Default table headers and summary footers. Reset follows the list surface.",
    ),
];

// Preserve the native row identity across the typed property-list migration.
pub(super) const VARIANT_MODE_ROW: &str = "__variant_mode__";

pub(super) const COLOR_ROW_HEIGHT: Rems = rems(5.5);

pub(super) fn color_label(token: &str) -> String {
    ESSENTIAL_COLORS
        .iter()
        .find(|(key, _, _)| *key == token)
        .map_or_else(|| colors::label(token), |(_, label, _)| (*label).to_owned())
}

pub(super) fn variant_label(family: &ThemeFamily, variant: &ThemeVariant) -> String {
    family
        .suffix(variant.id())
        .filter(|suffix| !suffix.is_empty())
        .unwrap_or_else(|| {
            if variant.mode().is_dark() {
                "Dark"
            } else {
                "Light"
            }
        })
        .to_owned()
}

const fn font_label(role: FontRole) -> &'static str {
    match role {
        FontRole::Interface => "Interface",
        FontRole::Reading => "Reading",
        FontRole::Headings => "Headings",
        FontRole::Code => "Code and editing",
    }
}

impl ThemeEditor {
    fn render_color_controls(
        id: u64,
        token: &str,
        label: &str,
        row: &ColorRow,
        active: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let value = if row.display.is_empty() {
            "Not set".to_owned()
        } else {
            row.display.clone()
        };
        let control = if let Some(controls) = row.controls.as_ref().filter(|_| active) {
            ColorPicker::new(&controls.picker)
                .small()
                .w_32()
                .label(value)
                .accessibility_label(format!("Pick {label} color"))
                .into_any_element()
        } else {
            Button::new(format!("color-value-{id}-{token}"))
                .small()
                .ghost()
                .w_32()
                .justify_start()
                .tooltip(format!("Choose {label} color…"))
                .accessibility_label(format!("Edit {label} color"))
                .children(
                    try_parse_color(&row.display)
                        .ok()
                        .map(|color| div().size_4().rounded_sm().bg(color)),
                )
                .label(value)
                .on_click(cx.listener({
                    let token = token.to_owned();
                    move |this, _, window, cx| this.edit_color(id, &token, window, cx)
                }))
                .into_any_element()
        };
        h_flex()
            .gap_2()
            .child(control)
            .child(
                Button::new(format!("reset-color-{id}-{token}"))
                    .small()
                    .ghost()
                    .icon(IconName::Undo)
                    .tooltip("Use automatic color")
                    .accessibility_label(format!("Reset {label} to automatic"))
                    .disabled(row.valid.is_none())
                    .on_click(cx.listener({
                        let token = token.to_owned();
                        move |this, _, window, cx| this.reset_color(id, &token, window, cx)
                    })),
            )
            .into_any_element()
    }

    fn render_color_row(&self, id: u64, token: &str, cx: &Context<Self>) -> AnyElement {
        let Some(row) = self.controls.colors.get(token) else {
            return div().into_any_element();
        };
        let label = color_label(token);
        let active = self
            .active_color
            .as_ref()
            .is_some_and(|(variant, key)| *variant == id && key == token);
        let source = colors::origin(
            self.preview
                .as_ref()
                .is_some_and(|appearance| appearance.color_is_defined(token)),
        )
        .label();
        let description = if self.category == "Advanced" {
            colors::description(token).to_owned()
        } else {
            ESSENTIAL_COLORS
                .iter()
                .find(|(key, _, _)| *key == token)
                .map_or_else(String::new, |(_, _, description)| (*description).to_owned())
        };
        let value = Self::render_color_controls(id, token, &label, row, active, cx);

        v_flex()
            .id(format!("theme-token-{id}-{token}"))
            .w_full()
            .px_4()
            .py_3()
            .gap_1()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .id(format!("theme-token-name-{id}-{token}"))
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(token.to_owned()),
            )
            .child(
                h_flex()
                    .w_full()
                    .gap_3()
                    .flex_wrap()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(div().text_sm().child(label))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(description),
                            ),
                    )
                    .child(
                        v_flex().gap_1().items_end().child(value).child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(source),
                        ),
                    ),
            )
            .into_any_element()
    }

    fn render_variant_mode_row(id: u64, cx: &Context<Self>) -> AnyElement {
        let mode = cx
            .global::<ThemeLibrary>()
            .variant_by_id(id)
            .map(ThemeVariant::mode);
        v_flex()
            .id(format!("theme-token-{id}-{VARIANT_MODE_ROW}"))
            .w_full()
            .px_4()
            .py_3()
            .gap_1()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .w_full()
                    .gap_3()
                    .flex_wrap()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(div().text_sm().child("Appearance"))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Whether this variant is used in light or dark mode"),
                            ),
                    )
                    .child(
                        h_flex().gap_1().children(
                            [
                                ("variant-light", "Light", ThemeMode::Light),
                                ("variant-dark", "Dark", ThemeMode::Dark),
                            ]
                            .into_iter()
                            .map(|(key, label, value)| {
                                Button::new((key, id))
                                    .small()
                                    .ghost()
                                    .label(label)
                                    .selected(mode == Some(value))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.change_variant_mode(id, value, window, cx);
                                    }))
                            }),
                        ),
                    ),
            )
            .into_any_element()
    }

    fn render_color_group_heading(id: u64, group: &str, cx: &Context<Self>) -> AnyElement {
        div()
            .id(format!("theme-color-group-{id}-{group}"))
            .w_full()
            .px_4()
            .pt_3()
            .pb_1()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .text_color(cx.theme().muted_foreground)
            .child(group.to_owned())
            .into_any_element()
    }

    fn render_color_origin_filter(
        origin: colors::ColorOrigin,
        selected: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        Checkbox::new(format!("color-origin-{}", origin.id()))
            .small()
            .checked(selected)
            .label(origin.label())
            .on_click(cx.listener(move |this, checked, _, cx| {
                this.set_color_origin(origin, *checked, cx);
            }))
            .into_any_element()
    }

    fn render_fonts(&self, id: u64, cx: &Context<Self>) -> impl IntoElement {
        v_flex().w_full().gap_4().p_4()
            .child(div().text_sm().text_color(cx.theme().muted_foreground).child("Fonts by interface role"))
            .children(FontRole::ALL.into_iter().zip(&self.controls.fonts)
                .map(|(role, state)| {
                    let unavailable = cx.global::<ThemeLibrary>().variant_by_id(id)
                        .and_then(|v| v.document().font(role)).filter(|name| !cx.global::<FontCatalog>().contains(name));
                    let font = self.preview.as_ref().map(|appearance| appearance.font(role).clone()).unwrap_or_default();
                    let sample = match role {
                        FontRole::Interface => "Search notes, open a tab, make it yours.",
                        FontRole::Reading => "The quick brown fox jumps over the lazy dog.",
                        FontRole::Headings => "A place for your ideas",
                        FontRole::Code => "let answer = 42; // Aa Bb 0123456789",
                    };
                    v_flex().w_full().gap_2()
                        .child(div().text_sm().child(font_label(role)))
                        .child(div().id(("font-sample", role.index())).text_sm().text_color(cx.theme().muted_foreground).font_family(font).child(sample))
                        .child(Select::new(state).small().w_full().search_placeholder("Search fonts").accessibility_label(font_label(role)))
                        .children(unavailable.map(|font| div().text_sm().text_color(cx.theme().muted_foreground)
                            .child(format!("{font} is unavailable; the preview shows the fallback font."))))
                }))
            .child(Button::new(("apply-family-fonts", id)).small().label("Apply fonts to all variants")
                .on_click(cx.listener(move |this, _, window, cx| this.apply_fonts_to_all(id, window, cx))))
    }

    fn render_category_tabs(&self, cx: &Context<Self>) -> impl IntoElement {
        let id = self.edited;
        h_flex()
            .p_4()
            .gap_1()
            .flex_wrap()
            .border_b_1()
            .border_color(cx.theme().border)
            .children(["Colors", "Fonts", "Advanced"].into_iter().map(|category| {
                Button::new(format!("token-group-{id}-{category}"))
                    .small()
                    .ghost()
                    .selected(self.category == category)
                    .label(category)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.set_category(category, window, cx);
                    }))
            }))
    }

    fn render_navigation(&self, family: &ThemeFamily, cx: &Context<Self>) -> impl IntoElement {
        v_flex()
            .id("theme-variant-navigation")
            .w(rems(13.))
            .flex_none()
            .min_h_0()
            .border_r_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .p_3()
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
                    .gap_1()
                    .children(family.variants().iter().map(|variant| {
                        let id = variant.id();
                        let rename = self
                            .rename
                            .as_ref()
                            .filter(|session| session.target == RenameTarget::Variant(id));
                        h_flex()
                            .id(("theme-variant-row", id))
                            .w_full()
                            .gap_1()
                            .child(if let Some(session) = rename {
                                Input::new(&session.input)
                                    .id(format!("variant-name-input-{id}"))
                                    .small()
                                    .flex_1()
                                    .min_w_0()
                                    .aria_label("Variant name")
                                    .into_any_element()
                            } else {
                                let label = variant_label(family, variant);
                                Button::new(("select-variant", id))
                                    .small()
                                    .ghost()
                                    .flex_1()
                                    .min_w_0()
                                    .selected(id == self.edited)
                                    .accessibility_label(label.clone())
                                    .child(div().w_full().truncate().text_left().child(label))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.select(id, window, cx);
                                    }))
                                    .into_any_element()
                            })
                            .children(rename.is_none().then(|| {
                                h_flex()
                                    .gap_1()
                                    .child(
                                        Button::new(("rename-variant", id))
                                            .small()
                                            .ghost()
                                            .icon(Icon::new(DatalithIcon::Pen))
                                            .tooltip("Rename variant")
                                            .accessibility_label("Rename variant")
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.start_variant_rename(id, window, cx);
                                            })),
                                    )
                                    .child(
                                        Button::new(("remove-variant", id))
                                            .small()
                                            .ghost()
                                            .icon(IconName::Close)
                                            .tooltip("Delete variant")
                                            .accessibility_label("Delete variant")
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.remove_variant(id, window, cx);
                                            })),
                                    )
                            }))
                    })),
            )
            .child(
                div().p_2().child(
                    Button::new("add-variant")
                        .small()
                        .ghost()
                        .icon(IconName::Plus)
                        .label("Add variant…")
                        .on_click(cx.listener(|this, _, window, cx| this.add_variant(window, cx))),
                ),
            )
    }

    fn render_properties(&self, cx: &Context<Self>) -> AnyElement {
        let id = self.edited;
        if self.category == "Fonts" {
            return div()
                .relative()
                .flex_1()
                .min_h_0()
                .child(
                    div()
                        .id("theme-controls-scroll")
                        .absolute()
                        .inset_0()
                        .overflow_y_scroll()
                        .track_scroll(&self.scroll)
                        .child(self.render_fonts(id, cx)),
                )
                .vertical_scrollbar(&self.scroll)
                .into_any_element();
        }
        let visible_color_count = self
            .property_rows
            .iter()
            .filter(|row| matches!(row, PropertyRow::Color(_)))
            .count();
        v_flex()
            .w_full()
            .flex_1()
            .min_h_0()
            .child(self.render_color_header(visible_color_count, cx))
            .child(self.render_color_list(cx))
            .when(visible_color_count == 0, |view| {
                view.child(div().p_4().text_sm().child("No colors match these filters"))
            })
            .into_any_element()
    }

    fn render_color_header(
        &self,
        visible_color_count: usize,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let advanced = self.category == "Advanced";
        let total = self.controls.colors.len();
        let all_groups = if advanced {
            "All families"
        } else {
            "All colors"
        };
        let filtered = advanced
            && (!self.color_query.read(cx).value().is_empty()
                || self
                    .color_group
                    .read(cx)
                    .selected_value()
                    .is_some_and(|group| group.as_str() != all_groups)
                || !self.color_origins.all_selected());
        v_flex()
            .w_full()
            .flex_none()
            .p_4()
            .gap_2()
            .child(
                h_flex()
                    .gap_2()
                    .child(div().flex_1().text_sm().child(if advanced {
                        format!("All theme colors · {visible_color_count} / {total}")
                    } else {
                        "Core colors".into()
                    }))
                    .when(filtered, |row| {
                        row.child(
                            Button::new("clear-color-filters")
                                .small()
                                .ghost()
                                .label("Clear filters")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.color_query.update(cx, |query, cx| {
                                        query.set_value("", window, cx);
                                    });
                                    this.color_group.update(cx, |group, cx| {
                                        group.set_selected_value(
                                            &all_groups.into(),
                                            window,
                                            cx,
                                        );
                                    });
                                    this.color_origins = ColorOriginFilters::default();
                                    this.refresh_color_list(cx);
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(if advanced {
                        "All color tokens in the theme schema"
                    } else {
                        "Base colors and common overrides. Reset restores each color's fallback; component-specific overrides in Advanced take priority."
                    }),
            )
            .when(advanced, |header| header.child(self.render_color_filters(cx)))
    }

    fn render_color_filters(&self, cx: &Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_1()
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(
                        div().w(rems(12.)).flex_none().child(
                            Select::new(&self.color_group)
                                .small()
                                .w_full()
                                .accessibility_label("Component family"),
                        ),
                    )
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(&self.color_query)
                                .id("theme-color-search")
                                .small()
                                .w_full()
                                .aria_label("Search all colors"),
                        ),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Color origin"),
            )
            .child(h_flex().w_full().gap_4().flex_wrap().children(
                colors::ColorOrigin::ALL.into_iter().map(|origin| {
                    Self::render_color_origin_filter(
                        origin,
                        self.color_origins.includes(origin),
                        cx,
                    )
                }),
            ))
    }

    fn render_color_list(&self, cx: &Context<Self>) -> impl IntoElement {
        let id = self.edited;
        let editor = cx.entity();
        let color_list = self.color_list.clone();
        let measured_width = color_list.viewport_bounds().size.width;
        let row_height = COLOR_ROW_HEIGHT.to_pixels(self.rem_size);
        div()
            .id("theme-controls-scroll")
            .relative()
            .flex_1()
            .min_h_0()
            .child(
                list(self.color_list.clone(), move |ix, _, cx| {
                    editor.update(cx, |editor, cx| {
                        editor.property_rows.get(ix).map_or_else(
                            || div().into_any_element(),
                            |row| match row {
                                PropertyRow::VariantMode => Self::render_variant_mode_row(id, cx),
                                PropertyRow::GroupHeading(group) => {
                                    Self::render_color_group_heading(id, group, cx)
                                }
                                PropertyRow::Color(token) => editor.render_color_row(id, token, cx),
                            },
                        )
                    })
                })
                .size_full(),
            )
            .child(
                canvas(
                    move |_, _, _| {
                        // GPUI clears all height hints on the first layout
                        // and width changes. Restore off-screen estimates
                        // after it measures the viewport, without resetting
                        // the scroll anchor or rendering the entire list.
                        if color_list.viewport_bounds().size.width != measured_width {
                            color_list.with_uniform_item_height(row_height);
                        }
                    },
                    |_, (), _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .vertical_scrollbar(&self.color_list)
    }

    fn render_editor_body(&self, narrow: bool, window: &Window, cx: &Context<Self>) -> AnyElement {
        let controls = v_flex()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .child(self.render_category_tabs(cx))
            .child(self.render_properties(cx));
        let preview = v_flex()
            .id("theme-preview-pane")
            .size_full()
            .min_w_0()
            .min_h_0()
            .when(!narrow, |pane| {
                pane.border_l_1().border_color(cx.theme().border)
            })
            .child(self.preview_pane.clone());
        if narrow {
            if self.show_preview {
                preview.into_any_element()
            } else {
                controls.into_any_element()
            }
        } else {
            div()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .child(
                    h_resizable("theme-editor-layout")
                        .child(resizable_panel().child(controls))
                        .child(
                            resizable_panel()
                                .size(rems(28.).to_pixels(window.rem_size()))
                                .size_range(
                                    rems(20.).to_pixels(window.rem_size())
                                        ..rems(55.).to_pixels(window.rem_size()),
                                )
                                .flex_none()
                                .child(preview),
                        ),
                )
                .into_any_element()
        }
    }

    fn render_family_title(&self, family: &ThemeFamily, cx: &Context<Self>) -> AnyElement {
        self.rename
            .as_ref()
            .filter(|session| session.target == RenameTarget::Family)
            .map_or_else(
                || {
                    h_flex()
                        .min_w_0()
                        .gap_1()
                        .child(div().text_lg().truncate().child(family.name().to_owned()))
                        .child(
                            Button::new("rename-theme")
                                .small()
                                .ghost()
                                .icon(Icon::new(DatalithIcon::Pen))
                                .tooltip("Rename theme")
                                .accessibility_label("Rename theme")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.start_family_rename(window, cx);
                                })),
                        )
                        .into_any_element()
                },
                |session| {
                    Input::new(&session.input)
                        .id("theme-name-input")
                        .small()
                        .w_64()
                        .max_w_full()
                        .aria_label("Theme name")
                        .into_any_element()
                },
            )
    }

    fn render_editor_header(
        &self,
        family: &ThemeFamily,
        narrow: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let status = match family.status() {
            SaveStatus::Saving => "Saving…".to_owned(),
            SaveStatus::Autosaved => "All changes saved".to_owned(),
            SaveStatus::Failed(error) => format!("Couldn’t save: {error}"),
        };
        h_flex()
            .gap_2()
            .p_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .flex_wrap()
            .child(self.render_family_title(family, cx))
            .child(div().flex_1())
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(status),
            )
            .children(matches!(family.status(), SaveStatus::Failed(_)).then(|| {
                Button::new("retry-theme-save")
                    .small()
                    .label("Retry")
                    .on_click(cx.listener(|this, _, _, cx| {
                        match cx.global_mut::<ThemeLibrary>().flush_family(this.family_id) {
                            Ok(()) => this.error = None,
                            Err(error) => this.error = Some(error.to_string()),
                        }
                        cx.notify();
                    }))
            }))
            .children(narrow.then(|| {
                Button::new("toggle-theme-preview")
                    .small()
                    .label(if self.show_preview {
                        "Back to editing"
                    } else {
                        "Preview"
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_preview = !this.show_preview;
                        cx.notify();
                    }))
            }))
    }
}

impl Render for ThemeEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.rem_size != window.rem_size() {
            self.rem_size = window.rem_size();
            let scroll_top = self.color_list.logical_scroll_top();
            self.reset_color_list_layout();
            self.color_list.scroll_to(scroll_top);
        }
        let Some(family) = cx.global::<ThemeLibrary>().family(self.family_id) else {
            return div()
                .p_4()
                .child("This theme was deleted")
                .into_any_element();
        };
        let narrow = window.viewport_size().width.as_f32() < window.rem_size().as_f32() * 80.;
        v_flex()
            .id("theme-editor")
            .size_full()
            .min_h_0()
            .min_w_0()
            .bg(cx.theme().background)
            .track_focus(&self.focus)
            .child(self.render_editor_header(family, narrow, cx))
            .children(self.error.as_ref().map(|error| {
                div()
                    .px_4()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(error.clone())
            }))
            .child(
                h_flex()
                    .items_stretch()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(self.render_navigation(family, cx))
                    .child(self.render_editor_body(narrow, window, cx)),
            )
            .into_any_element()
    }
}
