mod constants;
mod frontmatter;

use std::ops::Range;
use std::path::PathBuf;

use gpui::*;
use gpui_component::ActiveTheme;
use gpui_component::ChildElement;
use gpui_component::checkbox::Checkbox;
use gpui_component::table::{
    Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
};
use gpui_component::input::InputState;
use gpui_component::scroll::ScrollableElement;
use percent_encoding::percent_decode_str;

use crate::document::handler::{FileHandler, FileHandlerEvent};
use crate::document::markdown::{MarkdownBlock, MarkdownInline, parse_markdown};
use crate::ui::BASE_FONT_SIZE;

use constants::*;
use frontmatter::render_frontmatter;

pub(crate) struct MarkdownViewer {
    input: Entity<InputState>,
    file_path: PathBuf,
}

#[derive(Clone, Copy, Default)]
struct InlineStyle {
    bold: bool,
    italic: bool,
    code: bool,
}

impl MarkdownViewer {
    pub(crate) fn new(input: Entity<InputState>, file_path: PathBuf) -> Self {
        Self { input, file_path }
    }

    pub(crate) fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }

    fn render_image(&self, url: &str, alt: &str, grouped: bool, cx: &mut App) -> AnyElement {
        let base_font_size = BASE_FONT_SIZE as f32;
        let container = if grouped {
            div().min_w_0().flex_shrink_1().my_2()
        } else {
            div().w_full().my_2()
        };
        if !url.starts_with("http://") && !url.starts_with("https://") {
            let decoded = percent_decode_str(url).decode_utf8_lossy().to_string();
            let path = self
                .file_path
                .parent()
                .map(|parent| parent.join(&decoded))
                .unwrap_or_else(|| PathBuf::from(&decoded));
            if path.exists() {
                return container
                    .child(img(path).max_w(relative(1.)))
                    .into_any_element();
            }
        }

        container
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

    pub(crate) fn render(
        &self,
        handler: Entity<FileHandler>,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
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

        let document = parse_markdown(&content);
        let mut elements = Vec::new();
        let mut element_id = 0usize;
        if let Some(frontmatter) = &document.frontmatter {
            elements.push(render_frontmatter(
                frontmatter,
                base_font_size,
                handler.clone(),
                cx,
            ));
        }
        elements.extend(self.render_blocks(
            &document.blocks,
            0,
            &mut element_id,
            handler,
            window,
            cx,
        ));

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

    fn render_blocks(
        &self,
        blocks: &[MarkdownBlock],
        list_depth: usize,
        element_id: &mut usize,
        handler: Entity<FileHandler>,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<AnyElement> {
        blocks
            .iter()
            .enumerate()
            .map(|(index, block)| {
                let is_last = index + 1 == blocks.len();
                let next_is_paragraph = matches!(
                    blocks.get(index + 1),
                    Some(MarkdownBlock::Paragraph(_))
                );
                self.render_block(
                    block,
                    list_depth,
                    is_last,
                    next_is_paragraph,
                    element_id,
                    handler.clone(),
                    window,
                    cx,
                )
            })
            .collect()
    }

    fn render_block(
        &self,
        block: &MarkdownBlock,
        list_depth: usize,
        is_last: bool,
        next_is_paragraph: bool,
        element_id: &mut usize,
        handler: Entity<FileHandler>,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let base_font_size = BASE_FONT_SIZE as f32;
        match block {
            MarkdownBlock::Heading { level, content } => {
                let idx = (*level as usize).saturating_sub(1).min(5);
                let size = MD_HEADING_SIZES[idx];
                div()
                    .w_full()
                    .min_w_0()
                    .flex()
                    .flex_wrap()
                    .text_size(px(base_font_size * size))
                    .font_weight(FontWeight::BOLD)
                    .mt(px(MD_HEADING_MARGIN * size))
                    .mb(px(MD_HEADING_MARGIN * size))
                    .line_height(px(base_font_size * size * MD_LINE_HEIGHT))
                    .children(self.render_inlines(content, InlineStyle::default(), handler, cx))
                    .into_any_element()
            }
            MarkdownBlock::Paragraph(content) => div()
                .w_full()
                .min_w_0()
                .flex()
                .flex_wrap()
                .mb(px(if is_last {
                    0.0
                } else if list_depth > 0 && !next_is_paragraph {
                    0.0
                } else {
                    MD_PARAGRAPH_MARGIN
                }))
                .children(self.render_inlines(content, InlineStyle::default(), handler, cx))
                .into_any_element(),
            MarkdownBlock::BlockQuote(blocks) => div()
                .w_full()
                .min_w_0()
                .pl(px(MD_BLOCKQUOTE_PADDING))
                .border_l(px(MD_BLOCKQUOTE_BORDER))
                .border_color(cx.theme().border)
                .text_color(cx.theme().muted_foreground)
                .children(self.render_blocks(blocks, list_depth, element_id, handler, window, cx))
                .into_any_element(),
            MarkdownBlock::List { start, items } => {
                let ordered = start.is_some();
                let first = start.unwrap_or(1);
                let rows = items.iter().enumerate().map(|(index, item)| {
                    let marker = if let Some(checked) = item.task {
                        let id = *element_id;
                        *element_id += 1;
                        div()
                            .flex()
                            .items_center()
                            .flex_shrink_0()
                            .h(px(base_font_size * MD_LINE_HEIGHT))
                            .mr_1()
                            .child(
                                Checkbox::new(ElementId::NamedInteger(
                                    "md-task-check".into(),
                                    id as u64,
                                ))
                                .checked(checked),
                            )
                            .into_any_element()
                    } else if ordered {
                        div()
                            .child(format!("{}. ", first + index as u64))
                            .into_any_element()
                    } else {
                        div().child("\u{2022} ".to_string()).into_any_element()
                    };
                    div()
                        .flex()
                        .items_start()
                        .w_full()
                        .min_w_0()
                        .child(
                            div()
                                .flex()
                                .items_start()
                                .flex_shrink_0()
                                .child(MD_LIST_INDENT.repeat(list_depth))
                                .child(marker),
                        )
                        .child(div().flex_1().min_w_0().children(self.render_blocks(
                            &item.blocks,
                            list_depth + 1,
                            element_id,
                            handler.clone(),
                            window,
                            cx,
                        )))
                });
                div()
                    .w_full()
                    .children(rows)
                    .mb(px(if list_depth > 0 { 0.0 } else { MD_PARAGRAPH_MARGIN }))
                    .into_any_element()
            }
            MarkdownBlock::Table { headers, rows } => {
                let header_style = InlineStyle {
                    bold: true,
                    ..InlineStyle::default()
                };
                let header_row = TableRow::new().children(headers.iter().map(|cell| {
                    TableHead::new().children(self.render_inlines(
                        cell,
                        header_style,
                        handler.clone(),
                        cx,
                    ))
                }));
                let body = TableBody::new().children(rows.iter().map(|row| {
                    TableRow::new().children(row.iter().map(|cell| {
                        TableCell::new().children(self.render_inlines(
                            cell,
                            InlineStyle::default(),
                            handler.clone(),
                            cx,
                        ))
                    }))
                }));
                let table_ix = *element_id;
                *element_id += 1;
                Table::new()
                    .with_ix(table_ix)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(px(MD_CODE_BLOCK_RADIUS))
                    .mb(px(if list_depth > 0 { 0.0 } else { MD_PARAGRAPH_MARGIN }))
                    .child(TableHeader::new().child(header_row))
                    .child(body)
                    .into_any_element()
            }
            MarkdownBlock::Code { language, content } => {
                let label = language.as_ref().map(|language| {
                    div()
                        .text_size(px(base_font_size * MD_CODE_FONT_SCALE * 0.8))
                        .text_color(cx.theme().muted_foreground)
                        .child(language.clone())
                });
                div()
                    .bg(cx.theme().muted)
                    .font_family("monospace")
                    .text_size(px(base_font_size * MD_CODE_FONT_SCALE))
                    .rounded(px(MD_CODE_BLOCK_RADIUS))
                    .p(px(MD_CODE_BLOCK_PADDING))
                    .mb_2()
                    .overflow_x_scrollbar()
                    .children(label)
                    .child(content.clone())
                    .into_any_element()
            }
            MarkdownBlock::Rule => div()
                .w_full()
                .my_2()
                .border_t_1()
                .border_color(cx.theme().border)
                .into_any_element(),
        }
    }

    fn render_inlines(
        &self,
        inlines: &[MarkdownInline],
        style: InlineStyle,
        handler: Entity<FileHandler>,
        cx: &mut App,
    ) -> Vec<AnyElement> {
        let mut elements = Vec::new();
        let mut text = String::new();
        let mut highlights: Vec<(Range<usize>, HighlightStyle)> = Vec::new();
        let mut index = 0;

        fn flush(
            elements: &mut Vec<AnyElement>,
            text: &mut String,
            highlights: &mut Vec<(Range<usize>, HighlightStyle)>,
        ) {
            if text.is_empty() {
                return;
            }
            let styled = StyledText::new(SharedString::from(std::mem::take(text)))
                .with_highlights(std::mem::take(highlights));
            elements.push(div().min_w_0().child(styled).into_any_element());
        }

        while index < inlines.len() {
            if let Some(end) = adjacent_image_run_end(inlines, index) {
                flush(&mut elements, &mut text, &mut highlights);
                let images = inlines[index..end].iter().filter_map(|inline| {
                    let MarkdownInline::Image { url, alt } = inline else {
                        return None;
                    };
                    Some(self.render_image(url, alt, true, cx))
                });
                elements.push(
                    div()
                        .w_full()
                        .min_w_0()
                        .flex()
                        .gap_2()
                        .children(images)
                        .into_any_element(),
                );
                index = end;
                continue;
            }

            match &inlines[index] {
                MarkdownInline::Link { url, content } => {
                    flush(&mut elements, &mut text, &mut highlights);
                    let link_url = url.clone();
                    let handler_clone = handler.clone();
                    elements.push(
                        div()
                            .id(SharedString::from(format!("link-{url}")))
                            .flex()
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
                            .children(self.render_inlines(content, style, handler.clone(), cx))
                            .into_any_element(),
                    );
                }
                MarkdownInline::Image { url, alt } => {
                    flush(&mut elements, &mut text, &mut highlights);
                    elements.push(self.render_image(url, alt, false, cx));
                }
                MarkdownInline::Break => {
                    flush(&mut elements, &mut text, &mut highlights);
                    elements.push(div().w_full().into_any_element());
                }
                _ => {
                    self.append_inline_text(
                        std::slice::from_ref(&inlines[index]),
                        style,
                        cx,
                        &mut text,
                        &mut highlights,
                    );
                }
            }
            index += 1;
        }
        flush(&mut elements, &mut text, &mut highlights);
        elements
    }

    fn append_inline_text(
        &self,
        inlines: &[MarkdownInline],
        style: InlineStyle,
        cx: &App,
        text: &mut String,
        highlights: &mut Vec<(Range<usize>, HighlightStyle)>,
    ) {
        for inline in inlines {
            match inline {
                MarkdownInline::Text(value) => {
                    let start = text.len();
                    text.push_str(value);
                    highlights.push((start..text.len(), inline_highlight(style, cx)));
                }
                MarkdownInline::Code(value) => {
                    let start = text.len();
                    text.push_str(value);
                    highlights.push((
                        start..text.len(),
                        inline_highlight(InlineStyle { code: true, ..style }, cx),
                    ));
                }
                MarkdownInline::Strong(children) => self.append_inline_text(
                    children,
                    InlineStyle {
                        bold: true,
                        ..style
                    },
                    cx,
                    text,
                    highlights,
                ),
                MarkdownInline::Emphasis(children) => self.append_inline_text(
                    children,
                    InlineStyle {
                        italic: true,
                        ..style
                    },
                    cx,
                    text,
                    highlights,
                ),
                _ => {}
            }
        }
    }
}

fn adjacent_image_run_end(inlines: &[MarkdownInline], start: usize) -> Option<usize> {
    if !matches!(inlines.get(start), Some(MarkdownInline::Image { .. })) {
        return None;
    }

    let mut index = start + 1;
    let mut image_count = 1;
    while index < inlines.len() {
        match &inlines[index] {
            MarkdownInline::Image { .. } => {
                image_count += 1;
                index += 1;
            }
            MarkdownInline::Text(text) if text.chars().all(char::is_whitespace) => index += 1,
            _ => break,
        }
    }

    (image_count > 1).then_some(index)
}

fn inline_highlight(style: InlineStyle, cx: &App) -> HighlightStyle {
    let mut highlight = HighlightStyle::default();
    if style.bold {
        highlight.font_weight = Some(FontWeight::BOLD);
    }
    if style.italic {
        highlight.font_style = Some(FontStyle::Italic);
    }
    if style.code {
        highlight.background_color = Some(cx.theme().muted);
    }
    highlight
}

#[cfg(test)]
mod tests {
    use super::{MarkdownInline, adjacent_image_run_end};

    fn image(name: &str) -> MarkdownInline {
        MarkdownInline::Image {
            url: name.into(),
            alt: String::new(),
        }
    }

    #[test]
    fn adjacent_images_form_a_run_across_inline_whitespace() {
        let inlines = [
            image("a.png"),
            MarkdownInline::Text(" ".into()),
            image("b.png"),
        ];

        assert_eq!(adjacent_image_run_end(&inlines, 0), Some(3));
    }

    #[test]
    fn a_line_break_prevents_an_image_run() {
        let inlines = [image("a.png"), MarkdownInline::Break, image("b.png")];

        assert_eq!(adjacent_image_run_end(&inlines, 0), None);
    }

    #[test]
    fn intervening_text_prevents_an_image_run() {
        let inlines = [
            image("a.png"),
            MarkdownInline::Text(" some text ".into()),
            image("b.png"),
        ];

        assert_eq!(adjacent_image_run_end(&inlines, 0), None);
    }
}
