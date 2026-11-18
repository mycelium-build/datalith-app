use super::{
    Category, ColorRow, PropertyRow, RenameTarget, ThemeEditor, colors, regions::EditorRegions,
    variants::VARIANT_RAIL_WIDTH,
};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    color_picker::ColorPicker,
    h_flex,
    input::Input,
    resizable::{h_resizable, resizable_panel},
    scroll::ScrollableElement as _,
    select::Select,
    tab::{Tab, TabBar},
    tooltip::Tooltip,
    try_parse_color, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AnyView, Context, FontWeight, InteractiveElement as _, IntoElement, ParentElement,
    Rems, Render, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas, div,
    list, rems,
};

use crate::{
    app::themes::{SaveStatus, ThemeFamily, ThemeLibrary, ThemeVariant},
    ui::icons::DatalithIcon,
};

/// Token, label, description and core-color group.
pub(super) type EssentialColor = (&'static str, &'static str, &'static str, &'static str);

// Base roles first, then common overrides used by the bundled themes and Datalith.
pub(super) const ESSENTIAL_COLORS: &[EssentialColor] = &[
    (
        "background",
        "Background",
        "The main workspace and reading surface",
        colors::CORE_GROUP,
    ),
    (
        "foreground",
        "Text",
        "Body text and interface labels",
        colors::CORE_GROUP,
    ),
    (
        "muted.background",
        "Muted surface",
        "Subdued surfaces such as code blocks and note properties",
        colors::CORE_GROUP,
    ),
    (
        "border",
        "Borders",
        "Dividers between panels and controls",
        colors::CORE_GROUP,
    ),
    (
        "primary.background",
        "Primary",
        "Primary action background",
        colors::CORE_GROUP,
    ),
    (
        "secondary.background",
        "Secondary",
        "Secondary action background",
        colors::CORE_GROUP,
    ),
    (
        "primary.foreground",
        "Text on primary",
        "Text on primary actions and checkbox marks. Reset follows Text.",
        colors::PRIMARY_GROUP,
    ),
    (
        "primary.hover.background",
        "Primary hover",
        "Hovered primary actions. Reset derives from Primary and Background.",
        colors::PRIMARY_GROUP,
    ),
    (
        "primary.active.background",
        "Primary pressed",
        "Pressed primary actions. Reset darkens Primary.",
        colors::PRIMARY_GROUP,
    ),
    (
        "link",
        "Link",
        "Links in notes, properties and Base. Reset follows Primary.",
        colors::PRIMARY_GROUP,
    ),
    (
        "selection.background",
        "Text selection",
        "Selected text in fields and source editors. Reset follows Primary.",
        colors::PRIMARY_GROUP,
    ),
    (
        "list.active.background",
        "Selected row",
        "Selected files, Todo tasks and choices. Reset derives from Primary and Background.",
        colors::PRIMARY_GROUP,
    ),
    (
        "list.active.border",
        "Selected row border",
        "Selected choices and file context-menu outlines. Reset derives from Primary and Background.",
        colors::PRIMARY_GROUP,
    ),
    (
        "secondary.foreground",
        "Text on secondary",
        "Toolbar buttons and supporting surfaces. Reset follows Text.",
        colors::SECONDARY_GROUP,
    ),
    (
        "secondary.hover.background",
        "Secondary hover",
        "Custom title-bar button hover. Reset derives from Secondary and Background.",
        colors::SECONDARY_GROUP,
    ),
    (
        "secondary.active.background",
        "Secondary selected",
        "Selected or open toolbar buttons. Reset darkens Secondary.",
        colors::SECONDARY_GROUP,
    ),
    (
        "accent.background",
        "Accent",
        "Menus and general highlights. Reset follows Secondary.",
        colors::SECONDARY_GROUP,
    ),
    (
        "accent.foreground",
        "Text on accent",
        "Text on highlighted menu items. Reset follows Text.",
        colors::SECONDARY_GROUP,
    ),
    (
        "list.hover.background",
        "Hovered row",
        "Hovered files and choices. Reset follows Accent.",
        colors::SECONDARY_GROUP,
    ),
    (
        "scrollbar.thumb.background",
        "Scrollbar thumb",
        "Scrollbar handles. Reset follows Accent.",
        colors::SECONDARY_GROUP,
    ),
    (
        "muted.foreground",
        "Muted text",
        "Hints, metadata and completed tasks. Reset blends Muted surface and Text.",
        colors::SURFACES_GROUP,
    ),
    (
        "input.border",
        "Input border",
        "Text-field and select borders. Reset follows Borders.",
        colors::SURFACES_GROUP,
    ),
    (
        "ring",
        "Focus ring",
        "Focused text fields and controls. Reset follows the Blue palette color.",
        colors::SURFACES_GROUP,
    ),
    (
        "list.background",
        "List background",
        "List surface and default table background. Reset follows Background.",
        colors::SURFACES_GROUP,
    ),
    (
        "list.head.background",
        "List / table header",
        "Default table headers and summary footers. Reset follows the list surface.",
        colors::SURFACES_GROUP,
    ),
];

fn essential(token: &str) -> Option<&'static EssentialColor> {
    ESSENTIAL_COLORS.iter().find(|(key, ..)| *key == token)
}

pub(super) const COLOR_ROW_HEIGHT: Rems = rems(2.25);

pub(super) fn color_label(token: &str) -> String {
    essential(token).map_or_else(
        || colors::label(token),
        |(_, label, ..)| (*label).to_owned(),
    )
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

impl ThemeEditor {
    fn render_color_value(
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
        if let Some(controls) = row.controls.as_ref().filter(|_| active) {
            return ColorPicker::new(&controls.picker)
                .small()
                .w_32()
                .label(value)
                .accessibility_label(format!("Pick {label} color"))
                .into_any_element();
        }
        Button::new(format!("color-value-{id}-{token}"))
            .small()
            .ghost()
            .w_32()
            .justify_start()
            .tooltip(format!("Choose {label} color…"))
            .accessibility_label(format!("Edit {label} color"))
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(
                        div()
                            .size_4()
                            .flex_none()
                            .rounded_sm()
                            .border_1()
                            .border_color(cx.theme().border)
                            .when_some(try_parse_color(&row.display).ok(), |swatch, color| {
                                swatch.bg(color)
                            }),
                    )
                    .child(div().min_w_0().truncate().child(value)),
            )
            .on_click(cx.listener({
                let token = token.to_owned();
                move |this, _, window, cx| this.edit_color(id, &token, window, cx)
            }))
            .into_any_element()
    }

    pub(super) fn render_color_reset(
        id: u64,
        token: &str,
        label: &str,
        row: &ColorRow,
        cx: &Context<Self>,
    ) -> Button {
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
            }))
    }

    /// One line per token: label, value, origin and reset; the description
    /// is in the label's tooltip.
    pub(super) fn render_color_row(&self, id: u64, token: &str, cx: &Context<Self>) -> AnyElement {
        let Some(row) = self.controls.colors.get(token) else {
            return div().into_any_element();
        };
        let label = color_label(token);
        let all_colors = self.category == Category::AllColors;
        let origin = colors::origin(
            self.preview
                .as_ref()
                .is_some_and(|appearance| appearance.color_is_defined(token)),
        );
        let description = if all_colors {
            colors::description(token)
        } else {
            essential(token).map_or("", |(_, _, description, _)| description)
        };
        h_flex()
            .id(format!("theme-token-{id}-{token}"))
            .w_full()
            .gap_3()
            .px_4()
            .py_1()
            .child(
                h_flex()
                    .id(format!("theme-token-name-{id}-{token}"))
                    .flex_1()
                    .min_w_0()
                    .gap_2()
                    .child(div().min_w_0().truncate().text_sm().child(label.clone()))
                    .when(all_colors, |name| {
                        name.child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(token.to_owned()),
                        )
                    })
                    .when(!description.is_empty(), |name| {
                        name.tooltip(move |window, cx| Tooltip::new(description).build(window, cx))
                    }),
            )
            .child(Self::render_color_value(
                id,
                token,
                &label,
                row,
                self.is_active_color(token),
                cx,
            ))
            .child(
                div()
                    .w_8()
                    .flex_none()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(origin.short_label()),
            )
            .child(Self::render_color_reset(id, token, &label, row, cx))
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

    pub(super) fn render_category_tabs(&self, cx: &Context<Self>) -> impl IntoElement {
        let selected = Category::ALL
            .iter()
            .position(|category| *category == self.category)
            .unwrap_or_default();
        h_flex()
            .px_4()
            .py_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                TabBar::new("theme-editor-categories")
                    .segmented()
                    .small()
                    .selected_index(selected)
                    .children(Category::ALL.map(|category| Tab::new().label(category.label())))
                    .on_click(cx.listener(|this, ix: &usize, window, cx| {
                        if let Some(category) = Category::ALL.get(*ix) {
                            this.set_category(*category, window, cx);
                        }
                    })),
            )
    }

    pub(super) fn render_properties(&self, cx: &Context<Self>) -> AnyElement {
        let id = self.edited;
        if self.category == Category::Colors {
            return self.render_roles_list(cx).into_any_element();
        }
        if self.category == Category::Fonts {
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
        let total = self.controls.colors.len();
        let all_groups = self.category.all_groups();
        let filtered = !self.color_query.read(cx).value().is_empty()
            || self
                .color_group
                .read(cx)
                .selected_value()
                .is_some_and(|group| group.as_str() != all_groups)
            || !self.color_origins.all_selected();
        v_flex()
            .w_full()
            .flex_none()
            .p_4()
            .gap_2()
            .child(
                h_flex()
                    .gap_2()
                    .child(div().flex_1().text_sm().child(format!(
                        "All theme colors · {visible_color_count} / {total}"
                    )))
                    .when(filtered, |row| {
                        row.child(
                            Button::new("clear-color-filters")
                                .small()
                                .ghost()
                                .label("Clear filters")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.reset_color_filters(window, cx);
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("All color tokens in the theme schema"),
            )
            .child(self.render_color_filters(cx))
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

    fn render_roles_list(&self, cx: &Context<Self>) -> impl IntoElement {
        let id = self.edited;
        let editor = cx.entity();
        div()
            .id("theme-controls-scroll")
            .relative()
            .flex_1()
            .min_h_0()
            .child(
                list(self.roles_list.clone(), move |ix, _, cx| {
                    editor.update(cx, |editor, cx| editor.render_role_item(id, ix, cx))
                })
                .size_full(),
            )
            .vertical_scrollbar(&self.roles_list)
    }

    fn render_editor_body(
        &self,
        regions: &EditorRegions,
        narrow: bool,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let controls = AnyView::from(regions.controls.clone());
        let preview = v_flex()
            .id("theme-preview-pane")
            .size_full()
            .min_w_0()
            .min_h_0()
            .when(!narrow, |pane| {
                pane.border_l_1().border_color(cx.theme().border)
            })
            // Reuse the preview's last frame unless it changed itself, so
            // hovering or editing controls does not re-render its documents.
            .child(
                AnyView::from(self.preview_pane.clone())
                    .cached(StyleRefinement::default().size_full()),
            );
        if narrow {
            if self.show_preview {
                preview.into_any_element()
            } else {
                controls
                    .cached(StyleRefinement::default().w_full().flex_1().min_h_0())
                    .into_any_element()
            }
        } else {
            div()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .child(
                    h_resizable("theme-editor-layout")
                        .child(
                            resizable_panel()
                                .child(controls.cached(StyleRefinement::default().size_full())),
                        )
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
            self.roles_list.remeasure();
        }
        let Some(family) = cx.global::<ThemeLibrary>().family(self.family_id) else {
            return div()
                .p_4()
                .child("This theme was deleted")
                .into_any_element();
        };
        let Some(regions) = &self.regions else {
            return div().into_any_element();
        };
        let narrow = Self::is_narrow(window);
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
            .map(|editor| {
                if narrow {
                    editor
                        .child(regions.variants.clone())
                        .child(self.render_editor_body(regions, narrow, window, cx))
                } else {
                    editor.child(
                        h_flex()
                            .items_stretch()
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .child(
                                AnyView::from(regions.variants.clone()).cached(
                                    StyleRefinement::default()
                                        .w(VARIANT_RAIL_WIDTH)
                                        .h_full()
                                        .flex_none(),
                                ),
                            )
                            .child(self.render_editor_body(regions, narrow, window, cx)),
                    )
                }
            })
            .into_any_element()
    }
}
