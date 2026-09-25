use super::*;
use gpui_kit::base::TestSupportExt as _;
use gpui_kit::component::{
    ActiveTheme as _, IconName, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    color_picker::ColorPicker,
    h_flex,
    input::Input,
    scroll::ScrollableElement as _,
    select::Select,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Context, InteractiveElement as _, IntoElement, ParentElement, StatefulInteractiveElement as _,
    Styled as _, Window, div, rems,
};

use crate::app::{
    settings::FontRole,
    themes::{SaveStatus, ThemeFamily, ThemeLibrary},
};
use crate::ui::themes::preview::{PreviewFonts, ThemePreview};

fn group(token: &str) -> &'static str {
    if token.starts_with("sidebar.") || token.starts_with("tab.") || token.starts_with("title_bar.")
    {
        "Chrome"
    } else if token.starts_with("primary")
        || token.starts_with("accent")
        || token.starts_with("selection")
    {
        "Accent"
    } else if token.starts_with("highlight:") {
        "Syntax"
    } else {
        "Surface"
    }
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
    fn render_color_value(
        id: u64,
        token: &str,
        row: &ColorRow,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let color = gpui_kit::component::try_parse_color(&row.display).ok();
        div().w_full().py_1().child(
            Button::new(format!("color-value-{id}-{token}"))
                .small()
                .ghost()
                .w_full()
                .min_h_8()
                .accessibility_label(format!("Edit {token} color"))
                .child(
                    h_flex()
                        .w_full()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(token.replace('.', " · ")),
                        )
                        .children(color.map(|color| {
                            div()
                                .size_4()
                                .rounded_sm()
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(color)
                        }))
                        .child(div().w_32().child(row.display.clone())),
                )
                .on_click(cx.listener({
                    let token = token.to_owned();
                    move |this, _, window, cx| this.edit_color(id, &token, window, cx)
                })),
        )
    }

    fn render_color_row(&self, id: u64, token: &str, cx: &Context<Self>) -> impl IntoElement {
        let Some(row) = self
            .variants
            .get(&id)
            .and_then(|variant| variant.colors.get(token))
        else {
            return v_flex().into_any_element();
        };
        let color = gpui_kit::component::try_parse_color(&row.display).ok();
        let active = self
            .active_color
            .as_ref()
            .is_some_and(|(variant, key)| *variant == id && key == token);
        let Some(controls) = row.controls.as_ref().filter(|_| active) else {
            return Self::render_color_value(id, token, row, cx).into_any_element();
        };
        div()
            .id(format!("theme-token-{id}-{token}"))
            .w_full()
            .py_1()
            .child(
                h_flex()
                    .w_full()
                    .h_8()
                    .flex_shrink_0()
                    .min_w_0()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .truncate()
                            .child(token.replace('.', " · ")),
                    )
                    .children(color.map(|color| {
                        div()
                            .size_4()
                            .rounded_sm()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(color)
                    }))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div().w_32().child(
                                    Input::new(&controls.input)
                                        .id(format!("color-value-{id}-{token}"))
                                        .small()
                                        .aria_label(format!("{token} color")),
                                ),
                            )
                            .child(
                                ColorPicker::new(&controls.picker)
                                    .small()
                                    .accessibility_label(format!("Pick {token} color")),
                            ),
                    )
                    .child(
                        Button::new(format!("reset-color-{id}-{token}"))
                            .small()
                            .ghost()
                            .label("Reset")
                            .on_click(cx.listener({
                                let token = token.to_owned();
                                move |this, _, window, cx| this.reset_color(id, &token, window, cx)
                            })),
                    ),
            )
            .children(row.error.as_ref().map(|error| {
                div()
                    .mt_1()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(error.clone())
            }))
            .into_any_element()
    }

    fn render_group(&self, id: u64, name: &'static str, cx: &Context<Self>) -> impl IntoElement {
        let controls = self.variants.get(&id);
        let expanded = controls.is_some_and(|controls| controls.expanded.contains(name));
        div()
            .w_full()
            .mb_3()
            .child(
                Button::new(format!("token-group-{id}-{name}"))
                    .small()
                    .ghost()
                    .mb_1()
                    .icon(if expanded {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .label(name)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(controls) = this.variants.get_mut(&id) {
                            if !controls.expanded.insert(name) {
                                controls.expanded.remove(name);
                            }
                            cx.notify();
                        }
                    })),
            )
            .children(expanded.then(|| {
                div()
                    .w_full()
                    .pl_3()
                    .children(controls.into_iter().flat_map(|controls| {
                        controls
                            .colors
                            .keys()
                            .filter(|token| group(token) == name)
                            .map(|token| self.render_color_row(id, token, cx))
                    }))
            }))
    }

    fn render_fonts(&self, id: u64, cx: &Context<Self>) -> impl IntoElement {
        let controls = self.variants.get(&id);
        v_flex()
            .w_full()
            .gap_3()
            .child(
                h_flex()
                    .justify_between()
                    .gap_2()
                    .flex_wrap()
                    .child(div().child("Fonts"))
                    .child(
                        Button::new(("apply-family-fonts", id))
                            .small()
                            .ghost()
                            .label("Apply these fonts to all variants")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.apply_fonts_to_all(id, window, cx);
                            })),
                    ),
            )
            .children(
                controls
                    .into_iter()
                    .flat_map(|controls| FontRole::ALL.into_iter().zip(controls.fonts.iter()))
                    .map(|(role, state)| {
                        let selected = cx
                            .global::<ThemeLibrary>()
                            .variant_by_id(id)
                            .and_then(|v| v.document().font(role));
                        let unavailable =
                            selected.filter(|name| !cx.global::<FontCatalog>().contains(name));
                        v_flex()
                            .w_full()
                            .gap_1()
                            .child(div().text_sm().child(font_label(role)))
                            .child(
                                Select::new(state)
                                    .small()
                                    .w_full()
                                    .search_placeholder("Search fonts")
                                    .accessibility_label(font_label(role)),
                            )
                            .children(unavailable.map(|font| {
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!(
                                        "{font} is unavailable; using the Datalith fallback"
                                    ))
                            }))
                    }),
            )
    }

    #[allow(
        clippy::too_many_lines,
        reason = "A variant's header and expandable controls form a single rendered section"
    )]
    fn render_variant(
        &self,
        family: &ThemeFamily,
        id: u64,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let Some(controls) = self.variants.get(&id) else {
            return v_flex().into_any_element();
        };
        let Some(variant) = family.variants().iter().find(|v| v.id() == id) else {
            return v_flex().into_any_element();
        };
        let selected = id == self.edited;
        let expanded = self.expanded_variants.contains(&id);
        let dark = variant.mode().is_dark();
        div()
            .id(("variant-editor", id))
            .w_full()
            .min_w_0()
            .p_3()
            .border_1()
            .border_color(if selected {
                cx.theme().primary
            } else {
                cx.theme().border
            })
            .rounded(cx.theme().radius)
            .track_focus(&controls.header_focus)
            .tab_stop(false)
            .child(
                h_flex()
                    .w_full()
                    .mb_3()
                    .min_w_0()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        Button::new(("select-variant", id))
                            .small()
                            .ghost()
                            .icon(if expanded {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            })
                            .label(variant.name().to_owned())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.set_edited(id, window, cx);
                                if !this.expanded_variants.insert(id) {
                                    this.expanded_variants.remove(&id);
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(&controls.suffix)
                                .id(("variant-suffix", id))
                                .small()
                                .aria_label(format!("Suffix for {}", variant.name())),
                        ),
                    )
                    .child(
                        Button::new(("variant-light", id))
                            .small()
                            .ghost()
                            .selected(!dark)
                            .label("Light")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.change_variant_mode(
                                    id,
                                    gpui_kit::component::ThemeMode::Light,
                                    window,
                                    cx,
                                );
                            })),
                    )
                    .child(
                        Button::new(("variant-dark", id))
                            .small()
                            .ghost()
                            .selected(dark)
                            .label("Dark")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.change_variant_mode(
                                    id,
                                    gpui_kit::component::ThemeMode::Dark,
                                    window,
                                    cx,
                                );
                            })),
                    )
                    .child(
                        Button::new(("remove-variant", id))
                            .small()
                            .ghost()
                            .icon(IconName::Close)
                            .accessibility_label(format!("Remove {}", variant.name()))
                            .tooltip("Remove variant")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.remove_variant(id, window, cx);
                            })),
                    ),
            )
            .children(controls.suffix_error.as_ref().map(|error| {
                div()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(error.clone())
            }))
            .children(expanded.then(|| {
                div()
                    .w_full()
                    .children(
                        ["Surface", "Chrome", "Accent", "Syntax"]
                            .into_iter()
                            .map(|name| self.render_group(id, name, cx)),
                    )
                    .child(self.render_fonts(id, cx))
            }))
            .into_any_element()
    }
}

impl Render for ThemeEditor {
    #[allow(
        clippy::too_many_lines,
        reason = "The editor's header, controls and isolated preview share a single layout"
    )]
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let family = cx.global::<ThemeLibrary>().family(self.family_id);
        let Some(family) = family else {
            return div().child("This theme was deleted").into_any_element();
        };
        let status = match family.status() {
            SaveStatus::Saving => "Saving".to_owned(),
            SaveStatus::Autosaved => "Autosaved".to_owned(),
            SaveStatus::Failed(error) => format!("Couldn’t save: {error}"),
        };
        let preview = self.preview.as_ref().map(|appearance| {
            ThemePreview::new(
                appearance.theme(),
                PreviewFonts::new(
                    appearance.font(FontRole::Interface).clone(),
                    appearance.font(FontRole::Reading).clone(),
                    appearance.font(FontRole::Headings).clone(),
                    appearance.font(FontRole::Code).clone(),
                ),
            )
            .render()
            .into_any_element()
        });
        let narrow = window.viewport_size().width.as_f32() < window.rem_size().as_f32() * 75.;
        // This is a bounded editor viewport, so its content does not participate
        // in sizing the panes. Keep one native scroll owner and direct variant
        // children so additions can reveal their header by model position.
        let controls_content = div()
            .id("theme-controls-scroll")
            .absolute()
            .inset_0()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .children(
                family
                    .variants()
                    .iter()
                    .filter(|v| self.variants.contains_key(&v.id()))
                    .map(|variant| {
                        div()
                            .w_full()
                            .p_3()
                            .child(self.render_variant(family, variant.id(), cx))
                    }),
            );
        let controls = div()
            .relative()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .child(controls_content)
            .vertical_scrollbar(&self.scroll);
        let preview_pane = v_flex()
            .id("theme-preview-pane")
            .test_support()
            .w(rems(28.))
            .flex_shrink_0()
            .max_w_full()
            .min_w_0()
            .min_h_0()
            .border_color(cx.theme().border)
            .when(narrow, |pane| pane.w_full().flex_1())
            .when(!narrow, gpui_kit::Styled::border_l_1)
            .child(
                div().p_3().child(
                    Select::new(&self.selector)
                        .w_full()
                        .small()
                        .accessibility_label("Preview variant"),
                ),
            )
            .children(preview);
        let body = if narrow {
            if self.show_preview {
                preview_pane.into_any_element()
            } else {
                controls.into_any_element()
            }
        } else {
            h_flex()
                .items_stretch()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .child(controls)
                .child(preview_pane)
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
                    .w_full()
                    .min_w_0()
                    .gap_3()
                    .p_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .flex_wrap()
                    .child(div().text_lg().child(family.name().to_owned()))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "Custom · {} variants · {status}",
                                family.variants().len()
                            )),
                    )
                    .children(matches!(family.status(), SaveStatus::Failed(_)).then(|| {
                        Button::new("retry-theme-save")
                            .small()
                            .label("Retry")
                            .on_click(cx.listener(|this, _, _, cx| {
                                match cx.global_mut::<ThemeLibrary>().retry(this.family_id) {
                                    Ok(()) => cx.notify(),
                                    Err(error) => {
                                        this.error = Some(error.to_string());
                                        cx.notify();
                                    }
                                }
                            }))
                    }))
                    .child(
                        Button::new("add-variant")
                            .small()
                            .label("Add variant")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.add_variant(window, cx)),
                            ),
                    )
                    .children(narrow.then(|| {
                        Button::new("toggle-theme-preview")
                            .small()
                            .ghost()
                            .label(if self.show_preview {
                                "Edit colors and fonts"
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
                    .px_3()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(error.clone())
            }))
            .child(body)
            .into_any_element()
    }
}
