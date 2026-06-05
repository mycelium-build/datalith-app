use std::path::PathBuf;

use gpui::*;
use gpui_component::ActiveTheme;
use gpui_component::input::{Input, InputEvent, InputState};
use percent_encoding::percent_decode_str;

use crate::consts::{
    BASE_FONT_SIZE, MD_BLOCKQUOTE_BORDER, MD_BLOCKQUOTE_PADDING, MD_CODE_BLOCK_PADDING,
    MD_CODE_BLOCK_RADIUS, MD_CODE_FONT_SCALE, MD_CODE_PADDING, MD_CODE_RADIUS,
    MD_FRONTMATTER_FONT_SCALE, MD_FRONTMATTER_MARGIN, MD_FRONTMATTER_PADDING,
    MD_FRONTMATTER_RADIUS, MD_HEADING_MARGIN, MD_HEADING_SIZES, MD_IMAGE_MAX_WIDTH, MD_LINE_HEIGHT,
    MD_LIST_INDENT,
};
use crate::markdown::{
    MarkdownBlock, MarkdownEvent, MarkdownStyle, find_link_at_offset, parse_markdown,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum EditorMode {
    Edit,
    Preview,
}

pub(crate) enum MarkdownEditorEvent {
    LinkClicked(String, bool),
}

pub(crate) struct MarkdownEditor {
    input: Entity<InputState>,
    mode: EditorMode,
    file_path: Option<PathBuf>,
    _sub: Option<Subscription>,
}

impl EventEmitter<MarkdownEditorEvent> for MarkdownEditor {}

struct ListItemData {
    marker_text: String,
    is_ordered: bool,
    indent: usize,
    elements: Vec<AnyElement>,
}

impl MarkdownEditor {
    pub(crate) fn new(
        input: Entity<InputState>,
        mode: EditorMode,
        file_path: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) -> Self {
        let sub = cx.subscribe(&input, |_this, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });

        Self {
            input,
            mode,
            file_path,
            _sub: Some(sub),
        }
    }

    pub(crate) fn input(&self) -> &Entity<InputState> {
        &self.input
    }

    pub(crate) fn is_editing(&self) -> bool {
        self.mode == EditorMode::Edit
    }

    pub(crate) fn toggle_editing(&mut self, cx: &mut Context<Self>) {
        self.mode = match self.mode {
            EditorMode::Edit => EditorMode::Preview,
            EditorMode::Preview => EditorMode::Edit,
        };
        cx.notify();
    }

    pub(crate) fn open_link_at_cursor(&self, cx: &mut Context<Self>) {
        let offset = self.input.read(cx).cursor();
        let text = self.input.read(cx).value().to_string();
        if let Some(url) = find_link_at_offset(&text, offset) {
            cx.emit(MarkdownEditorEvent::LinkClicked(url, true));
        }
    }

    fn render_image(&self, url: &str, alt: &str, cx: &mut Context<Self>) -> AnyElement {
        let base_font_size = BASE_FONT_SIZE as f32;

        if !url.starts_with("http://") && !url.starts_with("https://") {
            let decoded = percent_decode_str(url).decode_utf8_lossy().to_string();
            let path = self
                .file_path
                .as_ref()
                .and_then(|fp| fp.parent())
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

        // Fallback: muted bordered box with alt text
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

    fn render_preview(&self, cx: &mut Context<Self>) -> AnyElement {
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
        let mut current_line: Vec<AnyElement> = Vec::new();
        let mut paragraph_lines: Vec<AnyElement> = Vec::new();
        let mut in_paragraph = false;
        let mut link_counter: u64 = 0;

        for event in events {
            match event {
                MarkdownEvent::Text(text, style) => {
                    if let Some(ref mut li) = current_list_item {
                        if let Some(ref url) = current_link_url {
                            let url_clone = url.clone();
                            link_counter += 1;
                            let link_id =
                                SharedString::from(format!("link-{}-{}", link_counter, url_clone));
                            let mut link = div().child(text.clone());
                            link = apply_style_to_div(link, &style, cx);
                            let link = link.id(link_id).cursor_pointer().on_click(cx.listener(
                                move |_, event: &ClickEvent, _, cx| {
                                    cx.emit(MarkdownEditorEvent::LinkClicked(
                                        url_clone.clone(),
                                        event.modifiers().platform,
                                    ));
                                },
                            ));
                            li.elements.push(link.into_any_element());
                        } else {
                            let parts: Vec<&str> = text.split('\n').collect();
                            for part in parts {
                                if !part.is_empty() {
                                    let mut span = div().child(part.to_string());
                                    span = apply_style_to_div(span, &style, cx);
                                    li.elements.push(span.into_any_element());
                                }
                            }
                        }
                    } else if let Some(ref url) = current_link_url {
                        let url_clone = url.clone();
                        link_counter += 1;
                        let link_id =
                            SharedString::from(format!("link-{}-{}", link_counter, url_clone));
                        let mut link = div().child(text.clone());
                        link = apply_style_to_div(link, &style, cx);
                        let link = link.id(link_id).cursor_pointer().on_click(cx.listener(
                            move |_, event: &ClickEvent, _, cx| {
                                cx.emit(MarkdownEditorEvent::LinkClicked(
                                    url_clone.clone(),
                                    event.modifiers().platform,
                                ));
                            },
                        ));
                        current_line.push(link.into_any_element());
                    } else {
                        let parts: Vec<&str> = text.split('\n').collect();
                        for (i, part) in parts.iter().enumerate() {
                            if i > 0 && !current_line.is_empty() {
                                let wrapped = div()
                                    .flex()
                                    .flex_wrap()
                                    .items_start()
                                    .children(current_line.drain(..))
                                    .into_any_element();
                                if in_paragraph {
                                    paragraph_lines.push(wrapped);
                                } else {
                                    elements.push(wrapped);
                                }
                            }
                            if !part.is_empty() {
                                let mut span = div().child(part.to_string());
                                span = apply_style_to_div(span, &style, cx);
                                current_line.push(span.into_any_element());
                            }
                        }
                    }
                }
                MarkdownEvent::LinkStart(url) => {
                    current_link_url = Some(url);
                }
                MarkdownEvent::LinkEnd => {
                    current_link_url = None;
                }
                MarkdownEvent::BlockStart(block) => {
                    if !matches!(block, MarkdownBlock::Paragraph) {
                        if !current_line.is_empty() {
                            let wrapped = div()
                                .flex()
                                .flex_wrap()
                                .items_start()
                                .children(current_line.drain(..))
                                .into_any_element();
                            paragraph_lines.push(wrapped);
                        }
                        if !paragraph_lines.is_empty() {
                            let wrapped = div()
                                .flex()
                                .flex_col()
                                .children(paragraph_lines.drain(..))
                                .into_any_element();
                            elements.push(wrapped);
                        }
                        in_paragraph = false;
                    } else {
                        in_paragraph = true;
                    }
                    if !current_line.is_empty() {
                        let wrapped = div()
                            .flex()
                            .flex_wrap()
                            .items_start()
                            .children(current_line.drain(..))
                            .into_any_element();
                        elements.push(wrapped);
                    }
                    block_stack.push(block.clone());
                    match block {
                        MarkdownBlock::Heading => {}
                        MarkdownBlock::Paragraph => {}
                        MarkdownBlock::List(ordered, depth) => {
                            in_ordered_list = ordered;
                            if ordered {
                                while ordered_counters.len() <= depth {
                                    ordered_counters.push(0);
                                }
                                ordered_counters[depth - 1] = 0;
                            }
                        }
                        MarkdownBlock::ListItem(depth) => {
                            if current_list_item.is_some() {
                                if let Some(li) = current_list_item.take() {
                                    let marker = if li.is_ordered {
                                        format!("{}. ", li.marker_text)
                                    } else {
                                        "\u{2022} ".to_string()
                                    };
                                    let indent_str = MD_LIST_INDENT.repeat(li.indent);
                                    let full_marker = format!("{}{}", indent_str, marker);
                                    let marker_span = div().child(full_marker);

                                    let item_row = div()
                                        .flex()
                                        .flex_row()
                                        .items_start()
                                        .child(marker_span)
                                        .children(li.elements);
                                    elements.push(item_row.into_any_element());
                                }
                            }
                            let marker_text = if in_ordered_list {
                                while ordered_counters.len() <= depth {
                                    ordered_counters.push(0);
                                }
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
                            elements.push(
                                div()
                                    .pl(px(MD_BLOCKQUOTE_PADDING))
                                    .border_l(px(MD_BLOCKQUOTE_BORDER))
                                    .border_color(cx.theme().border)
                                    .text_color(cx.theme().muted_foreground)
                                    .mb_2()
                                    .into_any_element(),
                            );
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
                                    .child(code)
                                    .into_any_element(),
                            );
                        }
                        MarkdownBlock::Frontmatter(fm_content) => {
                            let lines: Vec<&str> = fm_content.lines().collect();

                            let mut max_key_len = 0;
                            for line in &lines {
                                if let Some(colon_pos) = line.find(':') {
                                    let key = &line[..colon_pos];
                                    max_key_len = max_key_len.max(key.len());
                                }
                            }

                            let font_size = base_font_size * MD_FRONTMATTER_FONT_SCALE;
                            let line_h = px(font_size * MD_LINE_HEIGHT);
                            let key_width = if max_key_len > 0 {
                                px(font_size * max_key_len as f32 * 0.6)
                            } else {
                                px(0.0)
                            };

                            let mut content_elements: Vec<AnyElement> = Vec::new();
                            for line in lines {
                                if let Some(colon_pos) = line.find(':') {
                                    let key = &line[..colon_pos];
                                    let value = line[colon_pos + 1..].trim();
                                    content_elements.push(
                                        div()
                                            .flex()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .w(key_width)
                                                    .flex_shrink_0()
                                                    .text_size(px(font_size))
                                                    .line_height(line_h)
                                                    .text_color(cx.theme().foreground)
                                                    .font_weight(FontWeight::BOLD)
                                                    .child(key.to_string()),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .text_size(px(font_size))
                                                    .line_height(line_h)
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(value.to_string()),
                                            )
                                            .into_any_element(),
                                    );
                                } else {
                                    content_elements.push(
                                        div()
                                            .text_size(px(font_size))
                                            .line_height(line_h)
                                            .text_color(cx.theme().muted_foreground)
                                            .child(line.to_string())
                                            .into_any_element(),
                                    );
                                }
                            }
                            elements.push(
                                div()
                                    .bg(cx.theme().tab_bar)
                                    .rounded(px(MD_FRONTMATTER_RADIUS))
                                    .p(px(MD_FRONTMATTER_PADDING))
                                    .mb(px(MD_FRONTMATTER_MARGIN))
                                    .children(content_elements)
                                    .into_any_element(),
                            );
                        }
                    }
                }
                MarkdownEvent::BlockEnd => {
                    if let Some(popped) = block_stack.pop() {
                        match popped {
                            MarkdownBlock::ListItem(_) => {
                                if let Some(li) = current_list_item.take() {
                                    let marker = if li.is_ordered {
                                        format!("{}. ", li.marker_text)
                                    } else {
                                        "\u{2022} ".to_string()
                                    };
                                    let indent_str = MD_LIST_INDENT.repeat(li.indent);
                                    let full_marker = format!("{}{}", indent_str, marker);
                                    let marker_span = div().child(full_marker);

                                    let item_row = div()
                                        .flex()
                                        .flex_row()
                                        .items_start()
                                        .child(marker_span)
                                        .children(li.elements);
                                    elements.push(item_row.into_any_element());
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
                                    let wrapped = div()
                                        .flex()
                                        .flex_wrap()
                                        .items_start()
                                        .children(current_line.drain(..))
                                        .into_any_element();
                                    paragraph_lines.push(wrapped);
                                }
                                if !paragraph_lines.is_empty() {
                                    let wrapped = div()
                                        .flex()
                                        .flex_col()
                                        .children(paragraph_lines.drain(..))
                                        .into_any_element();
                                    elements.push(wrapped);
                                }
                                in_paragraph = false;
                                elements.push(div().m_2().into_any_element());
                            }
                            MarkdownBlock::Heading => {
                                elements.push(div().m_2().into_any_element());
                            }
                            _ => {}
                        }
                    }
                }
                MarkdownEvent::Image { url, alt } => {
                    // Flush any pending inline content before the image
                    if !current_line.is_empty() {
                        let wrapped = div()
                            .flex()
                            .flex_wrap()
                            .items_start()
                            .children(current_line.drain(..))
                            .into_any_element();
                        if in_paragraph {
                            paragraph_lines.push(wrapped);
                        } else {
                            elements.push(wrapped);
                        }
                    }
                    if in_paragraph && !paragraph_lines.is_empty() {
                        let para = div()
                            .flex()
                            .flex_col()
                            .children(paragraph_lines.drain(..))
                            .into_any_element();
                        elements.push(para);
                        in_paragraph = false;
                    }
                    elements.push(self.render_image(&url, &alt, cx));
                }
            }
        }

        if let Some(li) = current_list_item.take() {
            let marker = if li.is_ordered {
                format!("{}. ", li.marker_text)
            } else {
                "\u{2022} ".to_string()
            };
            let indent_str = MD_LIST_INDENT.repeat(li.indent);
            let full_marker = format!("{}{}", indent_str, marker);
            let marker_span = div().child(full_marker);

            let item_row = div()
                .flex()
                .flex_row()
                .items_start()
                .child(marker_span)
                .children(li.elements);
            elements.push(item_row.into_any_element());
        }
        if !current_line.is_empty() {
            let wrapped = div()
                .flex()
                .flex_wrap()
                .items_start()
                .children(current_line.drain(..))
                .into_any_element();
            if in_paragraph {
                paragraph_lines.push(wrapped);
            } else {
                elements.push(wrapped);
            }
        }
        if !paragraph_lines.is_empty() {
            let wrapped = div()
                .flex()
                .flex_col()
                .children(paragraph_lines.drain(..))
                .into_any_element();
            elements.push(wrapped);
        }

        div()
            .id("markdown-preview")
            .size_full()
            .overflow_y_scroll()
            .overflow_x_hidden()
            .p_4()
            .whitespace_normal()
            .line_height(px(base_font_size * MD_LINE_HEIGHT))
            .child(div().children(elements))
            .into_any_element()
    }

    fn render_editing(&self, _cx: &mut Context<Self>) -> AnyElement {
        let base_font_size = BASE_FONT_SIZE as f32;

        div()
            .size_full()
            .child(
                Input::new(&self.input)
                    .h_full()
                    .appearance(false)
                    .text_size(px(base_font_size))
                    .line_height(px(base_font_size * MD_LINE_HEIGHT)),
            )
            .into_any_element()
    }
}

impl Focusable for MarkdownEditor {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl Render for MarkdownEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(if self.mode == EditorMode::Edit {
            self.render_editing(cx)
        } else {
            self.render_preview(cx)
        })
    }
}

fn apply_style_to_div(div: Div, style: &MarkdownStyle, cx: &App) -> Div {
    let base_font_size = BASE_FONT_SIZE as f32;
    let mut el = div;

    match style {
        MarkdownStyle::Heading(level) => {
            let idx = (*level as usize).saturating_sub(1).min(5);
            let size = MD_HEADING_SIZES[idx];
            let margin = MD_HEADING_MARGIN * size;
            el = el
                .text_size(px(base_font_size * size))
                .font_weight(FontWeight::BOLD)
                .mt(px(margin))
                .mb(px(margin))
                .line_height(px(base_font_size * size * MD_LINE_HEIGHT));
        }
        MarkdownStyle::Bold => {
            el = el.font_weight(FontWeight::BOLD);
        }
        MarkdownStyle::Italic => {
            el = el.italic();
        }
        MarkdownStyle::BoldItalic => {
            el = el.font_weight(FontWeight::BOLD).italic();
        }
        MarkdownStyle::Code => {
            el = el
                .bg(cx.theme().muted)
                .font_family("monospace")
                .text_size(px(base_font_size * MD_CODE_FONT_SCALE))
                .rounded(px(MD_CODE_RADIUS))
                .px(px(MD_CODE_PADDING));
        }
        MarkdownStyle::Link => {
            el = el.text_color(cx.theme().primary).underline();
        }
        MarkdownStyle::Normal => {}
    }

    el
}
