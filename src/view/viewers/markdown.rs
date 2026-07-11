use std::ops::Range;
use std::path::PathBuf;

use gpui::*;
use gpui_component::ActiveTheme;
use gpui_component::Disableable;
use gpui_component::checkbox::Checkbox;
use gpui_component::input::InputState;
use gpui_component::scroll::ScrollableElement;
use percent_encoding::percent_decode_str;

use crate::consts::{
    BASE_FONT_SIZE, MD_BLOCKQUOTE_BORDER, MD_BLOCKQUOTE_PADDING, MD_CODE_BLOCK_PADDING,
    MD_CODE_BLOCK_RADIUS, MD_CODE_FONT_SCALE, MD_FRONTMATTER_FONT_SCALE, MD_FRONTMATTER_MARGIN,
    MD_FRONTMATTER_PADDING, MD_FRONTMATTER_RADIUS, MD_HEADING_MARGIN, MD_HEADING_SIZES,
    MD_IMAGE_MAX_WIDTH, MD_LINE_HEIGHT, MD_LIST_INDENT,
};
use crate::markdown::{MarkdownBlock, MarkdownEvent, MarkdownStyle, parse_markdown};
use crate::view::file_handler::{FileHandler, FileHandlerEvent};

pub(crate) struct MarkdownViewer {
    input: Entity<InputState>,
    file_path: PathBuf,
}

struct ListItemData {
    marker_text: String,
    is_ordered: bool,
    indent: usize,
    elements: Vec<AnyElement>,
}

impl MarkdownViewer {
    pub(crate) fn new(input: Entity<InputState>, file_path: PathBuf) -> Self {
        Self { input, file_path }
    }

    pub(crate) fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }

    fn render_image(&self, url: &str, alt: &str, cx: &mut App) -> AnyElement {
        let base_font_size = BASE_FONT_SIZE as f32;

        if !url.starts_with("http://") && !url.starts_with("https://") {
            let decoded = percent_decode_str(url).decode_utf8_lossy().to_string();
            let path = self
                .file_path
                .parent()
                .map(|parent| parent.join(&decoded))
                .unwrap_or_else(|| PathBuf::from(&decoded));

            if path.exists() {
                return div()
                    .w_full()
                    .max_w(px(MD_IMAGE_MAX_WIDTH))
                    .my_2()
                    .child(img(path).w_full())
                    .into_any_element();
            }
        }

        div()
            .w_full()
            .max_w(px(MD_IMAGE_MAX_WIDTH))
            .my_2()
            .p_2()
            .rounded(px(4.))
            .border_1()
            .border_color(cx.theme().border)
            .text_color(cx.theme().muted_foreground)
            .text_size(px(base_font_size * 0.9))
            .child(if alt.is_empty() {
                format!("[image: {}]", url)
            } else {
                format!("[{}]", alt)
            })
            .into_any_element()
    }

    pub(crate) fn render(&self, handler: Entity<FileHandler>, cx: &mut App) -> AnyElement {
        let content = self.input.read(cx).value().to_string();
        let base_font_size = BASE_FONT_SIZE as f32;

        if content.is_empty() {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(cx.theme().muted_foreground)
                .child("Start writing markdown...")
                .into_any_element();
        }

        let events = parse_markdown(&content);
        let mut elements: Vec<AnyElement> = Vec::new();
        let mut ordered_counters: Vec<u32> = vec![0];
        let mut in_ordered_list = false;
        let mut current_list_item: Option<ListItemData> = None;
        let mut block_stack: Vec<MarkdownBlock> = Vec::new();
        let mut current_link_url: Option<String> = None;
        let mut current_link_text = String::new();
        let mut current_link_highlights: Vec<(Range<usize>, HighlightStyle)> = Vec::new();
        let mut current_line: Vec<AnyElement> = Vec::new();
        let mut paragraph_lines: Vec<AnyElement> = Vec::new();
        let mut in_paragraph = false;
        let mut blockquote_depth = 0usize;

        let mut text_buffer = TextBuffer::new();

        for event in events {
            match event {
                MarkdownEvent::Text(text, style) => {
                    if current_link_url.is_some() {
                        let start = current_link_text.len();
                        current_link_text.push_str(&text);
                        let end = current_link_text.len();
                        let highlight = build_highlight_style(&style, cx);
                        current_link_highlights.push((start..end, highlight));
                    } else {
                        let parts: Vec<&str> = text.split('\n').collect();
                        for (i, part) in parts.iter().enumerate() {
                            if i > 0 {
                                flush_inline(
                                    &mut text_buffer,
                                    &mut current_list_item,
                                    &mut current_line,
                                );
                                if !current_line.is_empty() {
                                    paragraph_lines.push(wrap_line(&mut current_line));
                                }
                            }
                            if !part.is_empty() {
                                text_buffer.push(part, &style, cx);
                            }
                        }
                    }
                }
                MarkdownEvent::LinkStart(url) => {
                    if current_list_item.is_some() {
                        current_link_url = Some(url);
                        current_link_text.clear();
                        current_link_highlights.clear();
                    } else {
                        if text_buffer.is_whitespace_only() {
                            text_buffer.clear();
                        } else {
                            flush_inline(
                                &mut text_buffer,
                                &mut current_list_item,
                                &mut current_line,
                            );
                        }
                        current_link_url = Some(url);
                        current_link_text.clear();
                        current_link_highlights.clear();
                    }
                }
                MarkdownEvent::LinkEnd => {
                    if let Some(url) = current_link_url.take() {
                        if current_list_item.is_some() {
                            let link_style = MarkdownStyle::Link;
                            text_buffer.push(&current_link_text, &link_style, cx);
                        } else if !current_link_text.is_empty() {
                            let link_styled =
                                StyledText::new(SharedString::from(current_link_text.clone()))
                                    .with_highlights(current_link_highlights.clone());
                            let handler_clone = handler.clone();
                            let link_url = url.clone();
                            let link_el = div()
                                .id(SharedString::from(format!("link-{}", url)))
                                .text_color(cx.theme().primary)
                                .underline()
                                .cursor_pointer()
                                .on_click(move |event: &ClickEvent, _window, cx| {
                                    handler_clone.update(cx, |_, cx| {
                                        cx.emit(FileHandlerEvent::LinkClicked(
                                            link_url.clone(),
                                            event.modifiers().platform,
                                        ));
                                    });
                                })
                                .child(link_styled)
                                .into_any_element();
                            current_line.push(link_el);
                        }
                        current_link_text.clear();
                        current_link_highlights.clear();
                    }
                }
                MarkdownEvent::BlockStart(block) => {
                    flush_inline(&mut text_buffer, &mut current_list_item, &mut current_line);
                    if !matches!(block, MarkdownBlock::Paragraph) {
                        if !current_line.is_empty() {
                            paragraph_lines.push(wrap_line(&mut current_line));
                        }
                        if !paragraph_lines.is_empty() {
                            elements.push(wrap_paragraph(
                                &mut paragraph_lines,
                                blockquote_depth,
                                cx,
                            ));
                        }
                        in_paragraph = false;
                    } else {
                        in_paragraph = true;
                    }
                    block_stack.push(block.clone());
                    match block {
                        MarkdownBlock::List(ordered, depth) => {
                            in_ordered_list = ordered;
                            if ordered {
                                ensure_counter_depth(&mut ordered_counters, depth);
                                ordered_counters[depth - 1] = 0;
                            }
                        }
                        MarkdownBlock::ListItem(depth) => {
                            if let Some(li) = current_list_item.take() {
                                elements.push(render_list_item(li));
                            }
                            let marker_text = if in_ordered_list {
                                ensure_counter_depth(&mut ordered_counters, depth);
                                ordered_counters[depth - 1] += 1;
                                format!("{}", ordered_counters[depth - 1])
                            } else {
                                String::new()
                            };
                            current_list_item = Some(ListItemData {
                                marker_text,
                                is_ordered: in_ordered_list,
                                indent: depth - 1,
                                elements: Vec::new(),
                            });
                        }
                        MarkdownBlock::BlockQuote => {
                            blockquote_depth += 1;
                        }
                        MarkdownBlock::Code(code) => {
                            elements.push(
                                div()
                                    .bg(cx.theme().muted)
                                    .font_family("monospace")
                                    .text_size(px(base_font_size * MD_CODE_FONT_SCALE))
                                    .rounded(px(MD_CODE_BLOCK_RADIUS))
                                    .p(px(MD_CODE_BLOCK_PADDING))
                                    .mb_2()
                                    .overflow_x_scrollbar()
                                    .child(code)
                                    .into_any_element(),
                            );
                        }
                        MarkdownBlock::Frontmatter(fm_content) => {
                            elements.push(render_frontmatter(
                                &fm_content,
                                base_font_size,
                                handler.clone(),
                                cx,
                            ));
                        }
                        _ => {}
                    }
                }
                MarkdownEvent::BlockEnd => {
                    flush_inline(&mut text_buffer, &mut current_list_item, &mut current_line);
                    if let Some(popped) = block_stack.pop() {
                        match popped {
                            MarkdownBlock::ListItem(_) => {
                                if let Some(li) = current_list_item.take() {
                                    elements.push(render_list_item(li));
                                }
                            }
                            MarkdownBlock::List(_, depth) => {
                                if in_ordered_list && depth > 0 && ordered_counters.len() > depth {
                                    ordered_counters.truncate(depth);
                                }
                                if depth == 1 {
                                    elements.push(div().m_2().into_any_element());
                                }
                            }
                            MarkdownBlock::Paragraph => {
                                if !current_line.is_empty() {
                                    paragraph_lines.push(wrap_line(&mut current_line));
                                }
                                if !paragraph_lines.is_empty() {
                                    elements.push(wrap_paragraph(
                                        &mut paragraph_lines,
                                        blockquote_depth,
                                        cx,
                                    ));
                                }
                                in_paragraph = false;
                                if blockquote_depth == 0 {
                                    elements.push(div().m_2().into_any_element());
                                }
                            }
                            MarkdownBlock::Heading => {
                                if !current_line.is_empty() {
                                    paragraph_lines.push(wrap_line(&mut current_line));
                                }
                                if !paragraph_lines.is_empty() {
                                    elements.push(wrap_paragraph(
                                        &mut paragraph_lines,
                                        blockquote_depth,
                                        cx,
                                    ));
                                }
                                elements.push(div().m_2().into_any_element());
                            }
                            MarkdownBlock::BlockQuote => {
                                blockquote_depth = blockquote_depth.saturating_sub(1);
                            }
                            _ => {}
                        }
                    }
                }
                MarkdownEvent::Image { url, alt } => {
                    flush_inline(&mut text_buffer, &mut current_list_item, &mut current_line);
                    if !current_line.is_empty() {
                        let wrapped = wrap_line(&mut current_line);
                        if in_paragraph {
                            paragraph_lines.push(wrapped);
                        } else {
                            elements.push(wrapped);
                        }
                    }
                    if in_paragraph && !paragraph_lines.is_empty() {
                        elements.push(wrap_paragraph(&mut paragraph_lines, blockquote_depth, cx));
                        in_paragraph = false;
                    }
                    elements.push(self.render_image(&url, &alt, cx));
                }
            }
        }

        flush_inline(&mut text_buffer, &mut current_list_item, &mut current_line);
        if let Some(li) = current_list_item.take() {
            elements.push(render_list_item(li));
        }
        if !current_line.is_empty() {
            let wrapped = wrap_line(&mut current_line);
            if in_paragraph {
                paragraph_lines.push(wrapped);
            } else {
                elements.push(wrapped);
            }
        }
        if !paragraph_lines.is_empty() {
            elements.push(wrap_paragraph(&mut paragraph_lines, blockquote_depth, cx));
        }

        div()
            .id("markdown-preview")
            .size_full()
            .overflow_y_scroll()
            .overflow_x_hidden()
            .p_4()
            .whitespace_normal()
            .line_height(px(base_font_size * MD_LINE_HEIGHT))
            .child(div().w_full().min_w_0().children(elements))
            .into_any_element()
    }
}

struct TextBuffer {
    text: String,
    highlights: Vec<(Range<usize>, HighlightStyle)>,
    heading_level: Option<u32>,
}

impl TextBuffer {
    fn new() -> Self {
        Self {
            text: String::new(),
            highlights: Vec::new(),
            heading_level: None,
        }
    }

    fn push(&mut self, text: &str, style: &MarkdownStyle, cx: &App) {
        if text.is_empty() {
            return;
        }
        let start = self.text.len();
        self.text.push_str(text);
        let end = self.text.len();
        let highlight = build_highlight_style(style, cx);
        self.highlights.push((start..end, highlight));
        if let MarkdownStyle::Heading(level) = style {
            self.heading_level = Some(*level);
        }
    }

    fn flush(&mut self) -> Option<Self> {
        if self.text.is_empty() {
            return None;
        }
        Some(Self {
            text: std::mem::take(&mut self.text),
            highlights: std::mem::take(&mut self.highlights),
            heading_level: self.heading_level.take(),
        })
    }

    fn is_whitespace_only(&self) -> bool {
        !self.text.is_empty() && self.text.chars().all(|c| c.is_whitespace())
    }

    fn clear(&mut self) {
        self.text.clear();
        self.highlights.clear();
        self.heading_level = None;
    }
}

fn link_highlight_style(cx: &App) -> HighlightStyle {
    HighlightStyle {
        color: Some(cx.theme().primary),
        underline: Some(UnderlineStyle {
            thickness: px(1.0),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn build_highlight_style(style: &MarkdownStyle, cx: &App) -> HighlightStyle {
    match style {
        MarkdownStyle::Bold => HighlightStyle {
            font_weight: Some(FontWeight::BOLD),
            ..Default::default()
        },
        MarkdownStyle::Italic => HighlightStyle {
            font_style: Some(FontStyle::Italic),
            ..Default::default()
        },
        MarkdownStyle::BoldItalic => HighlightStyle {
            font_weight: Some(FontWeight::BOLD),
            font_style: Some(FontStyle::Italic),
            ..Default::default()
        },
        MarkdownStyle::Code => HighlightStyle {
            background_color: Some(cx.theme().muted),
            ..Default::default()
        },
        MarkdownStyle::Link => link_highlight_style(cx),
        MarkdownStyle::Heading(_) => HighlightStyle {
            font_weight: Some(FontWeight::BOLD),
            ..Default::default()
        },
        MarkdownStyle::Normal => HighlightStyle::default(),
    }
}

struct FrontmatterProperty {
    key: String,
    values: Vec<String>,
}

fn parse_frontmatter_properties(content: &str) -> Vec<FrontmatterProperty> {
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

fn parse_frontmatter_link(value: &str) -> Option<(&str, &str)> {
    if let Some(link) = value
        .strip_prefix("[[")
        .and_then(|value| value.strip_suffix("]]"))
    {
        return Some(link.split_once('|').unwrap_or((link, link)));
    }

    let markdown = value.strip_prefix('[')?.strip_suffix(')')?;
    let (label, target) = markdown.split_once("](")?;
    Some((label, target))
}

fn render_frontmatter(
    content: &str,
    base_font_size: f32,
    handler: Entity<FileHandler>,
    cx: &App,
) -> AnyElement {
    let properties = parse_frontmatter_properties(content);
    let max_key_len = properties.iter().map(|p| p.key.len()).max().unwrap_or(0);

    let font_size = base_font_size * MD_FRONTMATTER_FONT_SCALE;
    let line_h = px(font_size * MD_LINE_HEIGHT);
    let key_width = if max_key_len > 0 {
        px(font_size * max_key_len as f32 * 0.6)
    } else {
        px(0.0)
    };

    let mut content_elements: Vec<AnyElement> = Vec::new();
    for (property_index, property) in properties.into_iter().enumerate() {
        let mut values: Vec<AnyElement> = Vec::new();
        for (value_index, value) in property.values.into_iter().enumerate() {
            let value_element = if matches!(value.as_str(), "true" | "false") {
                Checkbox::new(ElementId::NamedInteger(
                    "frontmatter-bool".into(),
                    (property_index * 1000 + value_index) as u64,
                ))
                .checked(value == "true")
                .disabled(true)
                .tab_stop(false)
                .into_any_element()
            } else if let Some((label, target)) = parse_frontmatter_link(&value) {
                let handler = handler.clone();
                let target = target.to_string();
                div()
                    .id(ElementId::NamedInteger(
                        "frontmatter-link".into(),
                        (property_index * 1000 + value_index) as u64,
                    ))
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
            } else {
                div().child(value).into_any_element()
            };
            values.push(value_element);
        }

        content_elements.push(
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
        .children(content_elements)
        .into_any_element()
}

fn ensure_counter_depth(counters: &mut Vec<u32>, depth: usize) {
    while counters.len() <= depth {
        counters.push(0);
    }
}

fn render_list_item(li: ListItemData) -> AnyElement {
    let marker = if li.is_ordered {
        format!("{}. ", li.marker_text)
    } else {
        "\u{2022} ".to_string()
    };
    let indent_str = MD_LIST_INDENT.repeat(li.indent);
    let full_marker = format!("{}{}", indent_str, marker);
    let marker_span = div().flex_shrink_0().child(full_marker);
    let content = div().flex_1().min_w_0().children(li.elements);
    div()
        .flex()
        .flex_row()
        .items_start()
        .child(marker_span)
        .w_full()
        .min_w_0()
        .flex_wrap()
        .child(content)
        .into_any_element()
}

fn wrap_line(current_line: &mut Vec<AnyElement>) -> AnyElement {
    div()
        .flex()
        .flex_wrap()
        .items_start()
        .w_full()
        .min_w_0()
        .children(current_line.drain(..))
        .into_any_element()
}

fn wrap_paragraph(
    paragraph_lines: &mut Vec<AnyElement>,
    blockquote_depth: usize,
    cx: &App,
) -> AnyElement {
    let mut paragraph = div()
        .flex()
        .flex_col()
        .w_full()
        .min_w_0()
        .children(paragraph_lines.drain(..))
        .into_any_element();
    for _ in 0..blockquote_depth {
        paragraph = div()
            .w_full()
            .min_w_0()
            .pl(px(MD_BLOCKQUOTE_PADDING))
            .border_l(px(MD_BLOCKQUOTE_BORDER))
            .border_color(cx.theme().border)
            .text_color(cx.theme().muted_foreground)
            .child(paragraph)
            .into_any_element();
    }
    paragraph
}

fn flush_inline(
    text_buffer: &mut TextBuffer,
    current_list_item: &mut Option<ListItemData>,
    current_line: &mut Vec<AnyElement>,
) {
    if let Some(element) = flush_text_buffer(text_buffer) {
        if let Some(li) = current_list_item {
            li.elements.push(element);
        } else {
            current_line.push(element);
        }
    }
}

fn flush_text_buffer(text_buffer: &mut TextBuffer) -> Option<AnyElement> {
    let data = text_buffer.flush()?;
    let styled = StyledText::new(SharedString::from(data.text)).with_highlights(data.highlights);
    let mut el = div().flex_1().min_w_0();
    if let Some(level) = data.heading_level {
        let base_font_size = BASE_FONT_SIZE as f32;
        let idx = (level as usize).saturating_sub(1).min(5);
        let size = MD_HEADING_SIZES[idx];
        let margin = MD_HEADING_MARGIN * size;
        el = el
            .text_size(px(base_font_size * size))
            .font_weight(FontWeight::BOLD)
            .mt(px(margin))
            .mb(px(margin))
            .line_height(px(base_font_size * size * MD_LINE_HEIGHT));
    }
    Some(el.child(styled).into_any_element())
}
