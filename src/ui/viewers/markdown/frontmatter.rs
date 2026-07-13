use gpui::*;
use gpui_component::checkbox::Checkbox;
use gpui_component::{ActiveTheme, Disableable};

use crate::document::handler::{FileHandler, FileHandlerEvent};
use crate::document::markdown::{Frontmatter, FrontmatterValue};

use super::constants::{
    MD_FRONTMATTER_FONT_SCALE, MD_FRONTMATTER_MARGIN, MD_FRONTMATTER_PADDING,
    MD_FRONTMATTER_RADIUS, MD_LINE_HEIGHT,
};

pub(super) fn render_frontmatter(
    frontmatter: &Frontmatter,
    base_font_size: f32,
    handler: Entity<FileHandler>,
    cx: &App,
) -> AnyElement {
    let max_key_len = frontmatter
        .properties
        .iter()
        .map(|property| property.key.len())
        .max()
        .unwrap_or(0);
    let font_size = base_font_size * MD_FRONTMATTER_FONT_SCALE;
    let line_h = px(font_size * MD_LINE_HEIGHT);
    let key_width = px(font_size * max_key_len as f32 * 0.6);
    let mut rows: Vec<AnyElement> = Vec::new();

    for (property_index, property) in frontmatter.properties.iter().enumerate() {
        let mut values: Vec<AnyElement> = Vec::new();
        for (value_index, value) in property.values.iter().enumerate() {
            let id = (property_index * 1000 + value_index) as u64;
            let element = match value {
                FrontmatterValue::Boolean(value) => {
                    Checkbox::new(ElementId::NamedInteger("frontmatter-bool".into(), id))
                        .checked(*value)
                        .disabled(true)
                        .tab_stop(false)
                        .into_any_element()
                }
                FrontmatterValue::Link { label, target } => {
                    render_link(label, target, id, handler.clone(), cx)
                }
                FrontmatterValue::Text(value) => div().child(value.clone()).into_any_element(),
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
                        .child(property.key.clone()),
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
