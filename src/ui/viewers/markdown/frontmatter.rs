use gpui::*;
use gpui_component::checkbox::Checkbox;
use gpui_component::{ActiveTheme, Disableable};

use crate::document::handler::{FileHandler, FileHandlerEvent};

use super::constants::{
    MD_FRONTMATTER_FONT_SCALE, MD_FRONTMATTER_MARGIN, MD_FRONTMATTER_PADDING,
    MD_FRONTMATTER_RADIUS, MD_LINE_HEIGHT,
};

struct FrontmatterProperty {
    key: String,
    values: Vec<String>,
}

fn parse_properties(content: &str) -> Vec<FrontmatterProperty> {
    let mut properties: Vec<FrontmatterProperty> = Vec::new();
    for line in content.lines() {
        if !line.starts_with(char::is_whitespace)
            && let Some((key, value)) = line.split_once(':')
        {
            properties.push(FrontmatterProperty {
                key: key.trim().to_string(),
                values: if value.trim().is_empty() {
                    Vec::new()
                } else {
                    vec![value.trim().to_string()]
                },
            });
        } else if let Some(property) = properties.last_mut() {
            let value = line.trim();
            if !value.is_empty() {
                property.values.push(value.to_string());
            }
        }
    }
    properties
}

fn parse_link(value: &str) -> Option<(&str, &str)> {
    if let Some(link) = value
        .strip_prefix("[[")
        .and_then(|value| value.strip_suffix("]]"))
    {
        return Some(link.split_once('|').unwrap_or((link, link)));
    }

    let markdown = value.strip_prefix('[')?.strip_suffix(')')?;
    markdown.split_once("](")
}

pub(super) fn render_frontmatter(
    content: &str,
    base_font_size: f32,
    handler: Entity<FileHandler>,
    cx: &App,
) -> AnyElement {
    let properties = parse_properties(content);
    let max_key_len = properties.iter().map(|p| p.key.len()).max().unwrap_or(0);
    let font_size = base_font_size * MD_FRONTMATTER_FONT_SCALE;
    let line_h = px(font_size * MD_LINE_HEIGHT);
    let key_width = px(font_size * max_key_len as f32 * 0.6);
    let mut rows: Vec<AnyElement> = Vec::new();

    for (property_index, property) in properties.into_iter().enumerate() {
        let mut values: Vec<AnyElement> = Vec::new();
        for (value_index, value) in property.values.into_iter().enumerate() {
            let id = (property_index * 1000 + value_index) as u64;
            let element = if matches!(value.as_str(), "true" | "false") {
                Checkbox::new(ElementId::NamedInteger("frontmatter-bool".into(), id))
                    .checked(value == "true")
                    .disabled(true)
                    .tab_stop(false)
                    .into_any_element()
            } else if let Some((label, target)) = parse_link(&value) {
                render_link(label, target, id, handler.clone(), cx)
            } else {
                div().child(value).into_any_element()
            };
            values.push(element);
        }

        rows.push(
            div()
                .flex()
                .items_start()
                .gap_1()
                .child(
                    div()
                        .w(key_width)
                        .flex_shrink_0()
                        .text_size(px(font_size))
                        .line_height(line_h)
                        .text_color(cx.theme().foreground)
                        .font_weight(FontWeight::BOLD)
                        .child(property.key),
                )
                .child(
                    div()
                        .flex_1()
                        .ml_2()
                        .flex()
                        .flex_col()
                        .text_size(px(font_size))
                        .line_height(line_h)
                        .text_color(cx.theme().muted_foreground)
                        .children(values),
                )
                .into_any_element(),
        );
    }

    div()
        .bg(cx.theme().tab_bar)
        .rounded(px(MD_FRONTMATTER_RADIUS))
        .p(px(MD_FRONTMATTER_PADDING))
        .mb(px(MD_FRONTMATTER_MARGIN))
        .children(rows)
        .into_any_element()
}

fn render_link(
    label: &str,
    target: &str,
    id: u64,
    handler: Entity<FileHandler>,
    cx: &App,
) -> AnyElement {
    let target = target.to_string();
    div()
        .id(ElementId::NamedInteger("frontmatter-link".into(), id))
        .text_color(cx.theme().primary)
        .underline()
        .cursor_pointer()
        .on_click(move |event: &ClickEvent, _window, cx| {
            handler.update(cx, |_, cx| {
                cx.emit(FileHandlerEvent::LinkClicked(
                    target.clone(),
                    event.modifiers().platform,
                ));
            });
        })
        .child(label.to_string())
        .into_any_element()
}
