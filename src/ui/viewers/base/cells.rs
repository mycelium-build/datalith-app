//! Cell rendering over projected snapshot values.

use gpui_kit::base::actions::{Cancel, SelectDown, SelectLeft, SelectRight, SelectUp};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme, StyledExt, Theme, h_flex, v_flex};
use gpui_kit::{
    AnyElement, App, ClickEvent, ElementId, Entity, InteractiveElement, IntoElement, KeyBinding,
    ParentElement, StatefulInteractiveElement, Styled, Subscription, div, prelude::FluentBuilder,
};

use crate::document::base::DisplayProperty;
use crate::document::handler::{FileHandler, FileHandlerEvent};

use super::snapshot::{BaseRow, BaseSnapshot, file_name, path_text};

const CELL_EDITOR_CONTEXT: &str = "BaseCellEditor";

/// Binds Escape to `Cancel` in a cell editor, after the field's own Escape,
/// and in the viewer; binds the arrows around a graph.
pub fn bind_keys(cx: &mut App) {
    let graph = Some(crate::app::keymap::BASE_GRAPH_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("escape", Cancel, Some(CELL_EDITOR_CONTEXT)),
        KeyBinding::new(
            "escape",
            Cancel,
            Some(crate::app::keymap::BASE_VIEWER_CONTEXT),
        ),
        KeyBinding::new("up", SelectUp, graph),
        KeyBinding::new("down", SelectDown, graph),
        KeyBinding::new("left", SelectLeft, graph),
        KeyBinding::new("right", SelectRight, graph),
    ]);
}

pub(super) struct CellEdit {
    pub(super) row: usize,
    pub(super) column: usize,
    pub(super) initial: String,
    pub(super) input: Entity<InputState>,
    #[allow(dead_code, reason = "for staying alive")]
    pub(super) subscription: Option<Subscription>,
    pub(super) error: Option<String>,
}

pub(super) fn editable_source(source: &str, has_frontmatter: bool) -> bool {
    if source.starts_with("file.") || source.starts_with("formula.") {
        return false;
    }
    has_frontmatter
}

pub(super) fn render_property_cell(
    snapshot: &BaseSnapshot,
    row: &BaseRow,
    property: &DisplayProperty,
    handler: &gpui_kit::WeakEntity<FileHandler>,
    id: ElementId,
    truncate: bool,
    theme: &Theme,
) -> AnyElement {
    if property.source == "file.name" {
        return render_link(
            id,
            file_name(&row.path).unwrap_or_default(),
            path_text(&row.path),
            truncate,
            handler.clone(),
            theme,
        );
    }
    let value = snapshot.projection_value(row, &property.source);
    let Some(value) = value else {
        return div()
            .id(id)
            .when(truncate, Styled::text_ellipsis)
            .whitespace_normal()
            .into_any_element();
    };
    if property.source == "file.links" {
        let targets = value
            .as_str()
            .map_or_default(|text| text.split(", ").map(str::to_string).collect::<Vec<_>>());
        let links = targets.iter().enumerate().map(|(index, target)| {
            render_link(
                ElementId::NamedInteger(
                    "base-link".into(),
                    u64::try_from(index).unwrap_or_default(),
                ),
                file_name(std::path::Path::new(target)).unwrap_or(target),
                target.clone(),
                false,
                handler.clone(),
                theme,
            )
        });
        return h_flex()
            .gap_1()
            .flex_wrap()
            .children(links)
            .into_any_element();
    }
    if let Some((label, target)) = wikilink_parts(value) {
        return render_link(id, &label, target, truncate, handler.clone(), theme);
    }
    div()
        .id(id)
        .when(truncate, Styled::text_ellipsis)
        .whitespace_normal()
        .child(format_scalar_text(value))
        .into_any_element()
}

/// Editable cell: single click swaps the static text for an inline input.
/// When `edit` matches this (row, column) the input renders instead.
/// `initial` is the current display text, captured for `begin_cell_edit`.
/// `view_entity` is `None` when the source is read-only (no edit affordance).
#[allow(clippy::too_many_arguments)]
pub(super) fn render_editable_cell(
    snapshot: &BaseSnapshot,
    row: &BaseRow,
    row_index: usize,
    column: usize,
    property: &DisplayProperty,
    handler: &gpui_kit::WeakEntity<FileHandler>,
    id: ElementId,
    truncate: bool,
    theme: &Theme,
    edit: Option<&CellEdit>,
    view_entity: &Entity<super::BaseViewState>,
    read_only: bool,
    has_frontmatter: bool,
) -> AnyElement {
    if let Some(active) = edit
        && active.row == row_index
        && active.column == column
    {
        let mut input = Input::new(&active.input)
            .appearance(false)
            .flex_1()
            .px_0()
            .py_0()
            .h_7()
            .h_flex()
            .text_base();
        if active.error.is_some() {
            input = input.border_color(theme.danger);
        }
        let entity = view_entity.clone();
        return v_flex()
            .id(id)
            .w_full()
            .key_context(CELL_EDITOR_CONTEXT)
            .on_action(move |_: &Cancel, _window, cx| {
                entity.update(cx, |state, cx| {
                    state.cancel_cell_edit(cx);
                });
            })
            .child(input)
            .when(active.error.is_some(), |flex| {
                flex.child(
                    div()
                        .text_xs()
                        .text_color(theme.danger)
                        .child(active.error.clone().unwrap_or_default()),
                )
            })
            .into_any_element();
    }
    let editable = !read_only
        && editable_source(&property.source, has_frontmatter)
        && !matches!(
            snapshot.projection_value(row, &property.source),
            Some(serde_json::Value::Array(_) | serde_json::Value::Object(_))
        );
    if !editable {
        return render_property_cell(snapshot, row, property, handler, id, truncate, theme);
    }
    let initial = cell_display_text(snapshot, row, property);
    let entity = view_entity.clone();
    let is_list = matches!(
        snapshot
            .definition
            .views
            .get(snapshot.view_index)
            .map(|view| view.view_type),
        Some(crate::document::base::ViewType::List)
    );
    div()
        .id(id)
        .when(truncate, Styled::text_ellipsis)
        .whitespace_normal()
        .min_w(gpui_kit::px(80.))
        .cursor_pointer()
        .hover(|style| style.bg(theme.muted))
        .on_click(move |_: &ClickEvent, window, cx| {
            entity.update(cx, |state, cx| {
                state.begin_cell_edit(is_list, row_index, column, &initial, window, cx);
            });
        })
        .child(render_property_cell_text(snapshot, row, property, theme))
        .into_any_element()
}

/// Plain-text value for a cell, without link interactivity
/// (used as the click-to-edit affordance underneath the edit wrapper).
fn render_property_cell_text(
    snapshot: &BaseSnapshot,
    row: &BaseRow,
    property: &DisplayProperty,
    theme: &Theme,
) -> AnyElement {
    if property.source == "file.name" {
        return div()
            .text_color(theme.link)
            .child(file_name(&row.path).unwrap_or_default().to_string())
            .into_any_element();
    }
    let text = cell_display_text(snapshot, row, property);
    div().child(text).into_any_element()
}

fn cell_display_text(snapshot: &BaseSnapshot, row: &BaseRow, property: &DisplayProperty) -> String {
    if property.source == "file.name" {
        return file_name(&row.path).unwrap_or_default().to_string();
    }
    snapshot
        .projection_value(row, &property.source)
        .map_or_default(format_scalar_text)
}

/// Splits `[[target|label]]`-shaped string values into interactive parts.
fn wikilink_parts(value: &serde_json::Value) -> Option<(String, String)> {
    let raw = value.as_str()?;
    let inner = raw.strip_prefix("[[")?.strip_suffix("]]")?;
    let (target, label) = inner.split_once('|').unwrap_or((inner, inner));
    Some((label.to_string(), target.to_string()))
}

/// Renders a scalar the way the compatibility matrix documents: lists as
/// comma-separated values, objects as compact YAML, nulls as empty cells.
pub(super) fn format_scalar_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Number(number) => format_number_text(number),
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Array(values) => values
            .iter()
            .map(format_scalar_text)
            .collect::<Vec<_>>()
            .join(", "),
        serde_json::Value::Object(_) => yaml_serde::to_string(value)
            .unwrap_or_default()
            .trim()
            .to_string(),
    }
}

fn format_number_text(number: &serde_json::Number) -> String {
    number
        .as_i64()
        .map(|value| value.to_string())
        .or_else(|| {
            number.as_f64().map(|value| {
                let rounded = (value * 1000.0).round() / 1000.0;
                if rounded.fract() == 0.0 && rounded.abs() < 1e15 {
                    format!("{rounded:.0}")
                } else {
                    format!("{rounded}")
                }
            })
        })
        .unwrap_or_default()
}

fn render_link(
    id: ElementId,
    label: &str,
    target: String,
    truncate: bool,
    handler: gpui_kit::WeakEntity<FileHandler>,
    theme: &Theme,
) -> AnyElement {
    div()
        .id(id)
        .when(truncate, Styled::text_ellipsis)
        .text_color(theme.link)
        .hover(|style| style.text_decoration_1().text_color(theme.link_hover))
        .active(|style| style.text_color(theme.link_active))
        .cursor_pointer()
        .on_click(move |event: &ClickEvent, _window, cx| {
            if let Some(handler) = handler.upgrade() {
                handler.update(cx, |_, cx| {
                    cx.emit(FileHandlerEvent::LinkClicked(
                        target.clone(),
                        event.modifiers().secondary(cx),
                    ));
                });
            }
        })
        .child(label.to_string())
        .into_any_element()
}

pub(super) fn centered_message(
    message: &str,
    cx: &gpui_kit::Context<super::BaseViewState>,
) -> AnyElement {
    gpui_kit::component::v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .text_color(cx.theme().muted_foreground)
        .child(message.to_string())
        .into_any_element()
}
