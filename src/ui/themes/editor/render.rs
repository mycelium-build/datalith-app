use super::*;
use gpui_kit::base::TestSupportExt as _;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    color_picker::ColorPicker,
    h_flex,
    input::Input,
    resizable::{h_resizable, resizable_panel},
    scroll::ScrollableElement as _,
    select::Select,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Context, InteractiveElement as _, IntoElement, ParentElement, StatefulInteractiveElement as _,
    Styled as _, Window, div, list,
};

use crate::{
    app::{
        settings::FontRole,
        themes::{SaveStatus, ThemeFamily, ThemeLibrary, ThemeVariant},
    },
    ui::icons::DatalithIcon,
};

// A small, ordered entry point. The complete theme schema remains editable in Advanced.
pub(super) const ESSENTIAL_COLORS: &[(&str, &str, &str)] = &[
    (
        "background",
        "Page background",
        "The main workspace and reading surface",
    ),
    ("foreground", "Text", "Body text and interface labels"),
    (
        "primary.background",
        "Primary accent",
        "Primary buttons, note links and highlighted controls",
    ),
    (
        "primary.foreground",
        "Text on accent",
        "Labels on primary buttons",
    ),
    (
        "muted.foreground",
        "Secondary text",
        "Hints and supporting information",
    ),
    ("border", "Borders", "Dividers between panels and controls"),
    (
        "sidebar.background",
        "Sidebar background",
        "The file navigation panel",
    ),
    (
        "sidebar.foreground",
        "Sidebar text",
        "File and folder names",
    ),
    (
        "tab.active.background",
        "Active tab",
        "The selected workspace tab",
    ),
    (
        "accent.background",
        "Selection accent",
        "Selected items and highlighted surfaces",
    ),
    (
        "muted.background",
        "Muted surface",
        "Code blocks and secondary surfaces",
    ),
];

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
    ) -> gpui_kit::AnyElement {
        if let Some(controls) = row.controls.as_ref().filter(|_| active) {
            h_flex()
                .gap_2()
                .child(
                    div()
                        .id(format!("color-picker-{id}-{token}"))
                        .test_support()
                        .child(
                            ColorPicker::new(&controls.picker)
                                .small()
                                .accessibility_label(format!("Pick {label} color")),
                        ),
                )
                .child(
                    Input::new(&controls.input)
                        .id(format!("color-value-{id}-{token}"))
                        .small()
                        .w_24()
                        .aria_label(format!("{label} color")),
                )
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
        } else {
            h_flex()
                .gap_2()
                .child(
                    Button::new(format!("color-picker-{id}-{token}"))
                        .small()
                        .outline()
                        .tooltip(format!("Choose {label} color…"))
                        .accessibility_label(format!("Pick {label} color"))
                        .children(
                            gpui_kit::component::try_parse_color(&row.display)
                                .ok()
                                .map(|color| div().size_4().rounded_sm().bg(color)),
                        )
                        .on_click(cx.listener({
                            let token = token.to_owned();
                            move |this, _, window, cx| this.edit_color(id, &token, true, window, cx)
                        })),
                )
                .child(
                    Button::new(format!("color-value-{id}-{token}"))
                        .small()
                        .ghost()
                        .w_24()
                        .label(if row.display.is_empty() {
                            "Not set".to_owned()
                        } else {
                            row.display.clone()
                        })
                        .accessibility_label(format!("Edit {label} color"))
                        .on_click(cx.listener({
                            let token = token.to_owned();
                            move |this, _, window, cx| {
                                this.edit_color(id, &token, false, window, cx);
                            }
                        })),
                )
                .into_any_element()
        }
    }

    fn render_color_row(&self, id: u64, token: &str, cx: &Context<Self>) -> gpui_kit::AnyElement {
        let Some(row) = self
            .variants
            .get(&id)
            .and_then(|variant| variant.colors.get(token))
        else {
            return div().into_any_element();
        };
        let label = color_label(token);
        let active = self
            .active_color
            .as_ref()
            .is_some_and(|(variant, key)| *variant == id && key == token);
        let source = if row.valid.is_some() {
            "In variant JSON"
        } else if self
            .preview
            .as_ref()
            .is_some_and(|appearance| appearance.color_is_defined(token))
        {
            "Datalith default"
        } else {
            "Component default"
        };
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
            .when(self.category == "Advanced", |row| {
                row.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{}  ·  {}", colors::group(token), token)),
                )
            })
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
            .children(row.error.as_ref().map(|error| {
                div()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(error.clone())
            }))
            .into_any_element()
    }

    fn render_fonts(&self, id: u64, cx: &Context<Self>) -> impl IntoElement {
        v_flex().w_full().gap_4().p_4()
            .child(div().text_sm().text_color(cx.theme().muted_foreground).child("Choose a font for each role. Default shows the font that will actually be used."))
            .children(self.variants.get(&id).into_iter().flat_map(|controls| FontRole::ALL.into_iter().zip(controls.fonts.iter()))
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

    fn render_variant_header(
        &self,
        family: &ThemeFamily,
        cx: &Context<Self>,
    ) -> gpui_kit::AnyElement {
        let id = self.edited;
        let Some(variant) = family.variants().iter().find(|v| v.id() == id) else {
            return div().into_any_element();
        };
        v_flex()
            .p_4()
            .gap_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .gap_3()
                    .flex_wrap()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(div().text_sm().child(variant_label(family, variant)))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Appearance defaults"),
                            ),
                    )
                    .child(
                        h_flex().gap_1().children(
                            [
                                (
                                    "variant-light",
                                    "Light",
                                    gpui_kit::component::ThemeMode::Light,
                                ),
                                ("variant-dark", "Dark", gpui_kit::component::ThemeMode::Dark),
                            ]
                            .into_iter()
                            .map(|(key, label, mode)| {
                                Button::new((key, id))
                                    .small()
                                    .ghost()
                                    .label(label)
                                    .selected(variant.mode() == mode)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.change_variant_mode(id, mode, window, cx);
                                    }))
                            }),
                        ),
                    ),
            )
            .child(h_flex().gap_1().flex_wrap().children(
                ["Colors", "Fonts", "Advanced"].into_iter().map(|category| {
                    Button::new(format!("token-group-{id}-{category}"))
                        .small()
                        .ghost()
                        .selected(self.category == category)
                        .label(category)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.category = category;
                            this.refresh_color_list(cx);
                            this.reset_property_scroll();
                            cx.notify();
                        }))
                }),
            ))
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

    fn render_rename_variant(id: u64, cx: &Context<Self>) -> Button {
        Button::new(("rename-variant", id))
            .small()
            .ghost()
            .icon(Icon::new(DatalithIcon::Pen))
            .tooltip("Rename variant")
            .accessibility_label("Rename variant")
            .on_click(cx.listener(move |this, _, window, cx| {
                this.select(id, window, cx);
                this.show_preview = false;
                crate::ui::settings::theme::dialogs::rename_variant(this.family_id, id, window, cx);
                cx.notify();
            }))
    }

    fn render_remove_variant(id: u64, cx: &Context<Self>) -> Button {
        Button::new(("remove-variant", id))
            .small()
            .ghost()
            .icon(IconName::Close)
            .tooltip("Delete variant")
            .accessibility_label("Delete variant")
            .on_click(cx.listener(move |this, _, window, cx| this.remove_variant(id, window, cx)))
    }

    fn render_navigation(&self, family: &ThemeFamily, cx: &Context<Self>) -> impl IntoElement {
        v_flex()
            .id("theme-variant-navigation")
            .w(gpui_kit::rems(13.))
            .flex_none()
            .min_h_0()
            .border_r_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .p_4()
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
                        h_flex()
                            .gap_1()
                            .child(
                                Button::new(("select-variant", id))
                                    .flex_1()
                                    .min_w_0()
                                    .small()
                                    .ghost()
                                    .selected(id == self.edited)
                                    .justify_start()
                                    .label(variant_label(family, variant))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.select(id, window, cx);
                                        this.reset_property_scroll();
                                    })),
                            )
                            .child(Self::render_rename_variant(id, cx))
                            .child(Self::render_remove_variant(id, cx))
                    }))
                    .child(div().mt_2().child(Self::render_add_variant(cx))),
            )
    }

    fn render_properties(&self, cx: &Context<Self>) -> gpui_kit::AnyElement {
        let id = self.edited;
        if self.category == "Fonts" {
            return div()
                .relative()
                .flex_1()
                .min_h_0()
                .child(
                    div()
                        .id("theme-controls-scroll")
                        .test_support()
                        .absolute()
                        .inset_0()
                        .overflow_y_scroll()
                        .track_scroll(&self.scroll)
                        .child(self.render_fonts(id, cx)),
                )
                .vertical_scrollbar(&self.scroll)
                .into_any_element();
        }
        let editor = cx.entity();
        let advanced = self.category == "Advanced";
        let total = self
            .variants
            .get(&id)
            .map_or(0, |variant| variant.colors.len());
        let filtered = !self.color_query.read(cx).value().is_empty()
            || self
                .color_group
                .read(cx)
                .selected_value()
                .is_some_and(|group| group.as_str() != "All areas");
        v_flex().w_full().flex_1().min_h_0()
            .child(v_flex().w_full().flex_none().p_4().gap_2()
                .child(h_flex().gap_2()
                    .child(div().flex_1().text_sm().child(if advanced { format!("All colors · {} / {total}", self.visible_colors.len()) } else { "Essential colors".into() }))
                    .when(advanced && filtered, |row| row.child(Button::new("clear-color-filters").small().ghost().label("Clear filters")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.color_query.update(cx, |query, cx| query.set_value("", window, cx));
                            this.color_group.update(cx, |group, cx| group.set_selected_value(&"All areas".into(), window, cx));
                            this.refresh_color_list(cx);
                            cx.notify();
                        })))))
                .child(div().text_xs().text_color(cx.theme().muted_foreground).child(if advanced {
                    "Browse by area or search a color. Includes unset colors. Source colors affect editing; they do not recolor the rendered note."
                } else {
                    "Start here to change the overall appearance. Advanced contains every component and source-editor color."
                }))
                .child(div().text_xs().text_color(cx.theme().muted_foreground).child("In variant JSON = explicit value. Datalith default = inherited from Light/Dark. Component default = calculated or unset. Reset removes the override."))
                .when(advanced, |view| view.child(h_flex().w_full().gap_2()
                    .child(div().w(gpui_kit::rems(12.)).flex_none().child(Select::new(&self.color_group).small().w_full().accessibility_label("Color area")))
                    .child(div().flex_1().min_w_0().child(Input::new(&self.color_query).id("theme-color-search").small().w_full().aria_label("Search all colors"))))))
            .child(div().id("theme-controls-scroll").test_support().relative().flex_1().min_h_0()
                .child(list(self.color_list.clone(), move |ix, _, cx| {
                    editor.update(cx, |editor, cx| {
                        editor.visible_colors.get(ix).map_or_else(|| div().into_any_element(), |token| editor.render_color_row(id, token, cx))
                    })
                }).size_full())
                .vertical_scrollbar(&self.color_list))
            .when(self.visible_colors.is_empty(), |view| view.child(div().p_4().text_sm().child("No colors match your search")))
            .into_any_element()
    }
}

impl Render for ThemeEditor {
    #[allow(
        clippy::too_many_lines,
        reason = "Composes the editor regions and the responsive preview"
    )]
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.rem_size != window.rem_size() {
            self.rem_size = window.rem_size();
            self.color_list.remeasure();
        }
        let Some(family) = cx.global::<ThemeLibrary>().family(self.family_id) else {
            return div()
                .p_4()
                .child("This theme was deleted")
                .into_any_element();
        };
        let status = match family.status() {
            SaveStatus::Saving => "Saving…".to_owned(),
            SaveStatus::Autosaved => "All changes saved".to_owned(),
            SaveStatus::Failed(error) => format!("Couldn’t save: {error}"),
        };
        let narrow = window.viewport_size().width.as_f32() < window.rem_size().as_f32() * 80.;
        let show_navigation =
            window.viewport_size().width.as_f32() >= window.rem_size().as_f32() * 65.;
        let id = self.edited;
        let controls = v_flex()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .child(self.render_variant_header(family, cx))
            .child(self.render_properties(cx));
        let preview = v_flex()
            .id("theme-preview-pane")
            .test_support()
            .size_full()
            .min_w_0()
            .min_h_0()
            .when(!narrow, |pane| {
                pane.border_l_1().border_color(cx.theme().border)
            })
            .child(self.preview_pane.clone());
        let body = if narrow {
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
                                .size(gpui_kit::rems(28.).to_pixels(window.rem_size()))
                                .size_range(
                                    gpui_kit::rems(20.).to_pixels(window.rem_size())
                                        ..gpui_kit::rems(55.).to_pixels(window.rem_size()),
                                )
                                .flex_none()
                                .child(preview),
                        ),
                )
                .into_any_element()
        };
        v_flex()
            .id("theme-editor")
            .size_full()
            .min_h_0()
            .min_w_0()
            .bg(cx.theme().background)
            .test_support()
            .track_focus(&self.focus)
            .child(
                h_flex()
                    .gap_2()
                    .p_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .flex_wrap()
                    .child(div().text_lg().truncate().child(family.name().to_owned()))
                    .child(
                        Button::new("rename-theme")
                            .small()
                            .ghost()
                            .icon(Icon::new(DatalithIcon::Pen))
                            .tooltip("Rename theme…")
                            .accessibility_label("Rename theme")
                            .on_click(cx.listener(|this, _, window, cx| {
                                crate::ui::settings::theme::dialogs::rename_family(
                                    this.family_id,
                                    window,
                                    cx,
                                );
                            })),
                    )
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
                                match cx.global_mut::<ThemeLibrary>().retry(this.family_id) {
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
                    })),
            )
            .children(self.error.as_ref().map(|error| {
                div()
                    .px_4()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(error.clone())
            }))
            .when(!show_navigation, |view| {
                view.child(
                    v_flex()
                        .p_3()
                        .gap_1()
                        .child(
                            h_flex()
                                .gap_2()
                                .child(div().text_sm().child("Variant"))
                                .child(
                                    Select::new(&self.selector)
                                        .flex_1()
                                        .small()
                                        .accessibility_label("Edited variant"),
                                )
                                .child(Self::render_rename_variant(id, cx))
                                .child(Self::render_remove_variant(id, cx)),
                        )
                        .child(Self::render_add_variant(cx)),
                )
            })
            .child(
                h_flex()
                    .items_stretch()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .when(show_navigation, |row| {
                        row.child(self.render_navigation(family, cx))
                    })
                    .child(body),
            )
            .into_any_element()
    }
}
