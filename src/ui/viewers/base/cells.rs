//! Cell rendering over projected snapshot values.

use gpui_kit::component::{ActiveTheme, h_flex};
use gpui_kit::{
    AnyElement, App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder,
};

use crate::document::base::DisplayProperty;
use crate::document::handler::{FileHandler, FileHandlerEvent};

use super::snapshot::{BaseRow, BaseSnapshot, file_name, path_text};

pub(super) fn render_property_cell(
    snapshot: &BaseSnapshot,
    row: &BaseRow,
    property: &DisplayProperty,
    handler: &gpui_kit::WeakEntity<FileHandler>,
    id: ElementId,
    truncate: bool,
    cx: &App,
) -> AnyElement {
    if property.source == "file.name" {
        return render_link(
            id,
            file_name(&row.path).unwrap_or_default(),
            path_text(&row.path),
            truncate,
            handler.clone(),
            cx,
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
            .map(|text| text.split(", ").map(str::to_string).collect::<Vec<_>>())
            .unwrap_or_default();
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
                cx,
            )
        });
        return h_flex()
            .gap_1()
            .flex_wrap()
            .children(links)
            .into_any_element();
    }
    if let Some((label, target)) = wikilink_parts(value) {
        return render_link(id, &label, target, truncate, handler.clone(), cx);
    }
    div()
        .id(id)
        .when(truncate, Styled::text_ellipsis)
        .whitespace_normal()
        .child(format_scalar_text(value))
        .into_any_element()
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
    cx: &App,
) -> AnyElement {
    div()
        .id(id)
        .when(truncate, Styled::text_ellipsis)
        .text_color(cx.theme().primary)
        .hover(Styled::underline)
        .cursor_pointer()
        .on_click(move |event: &ClickEvent, _window, cx| {
            if let Some(handler) = handler.upgrade() {
                handler.update(cx, |_, cx| {
                    cx.emit(FileHandlerEvent::LinkClicked(
                        target.clone(),
                        event.modifiers().secondary(),
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
