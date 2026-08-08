mod constants;
mod frontmatter;

use std::ops::Range;
use std::path::PathBuf;

use gpui::{
    AnyElement, App, ClickEvent, ElementId, Entity, FocusHandle, Focusable, FontStyle, FontWeight,
    HighlightStyle, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, StyledText, Window, div, img, px, relative,
};
use gpui_component::ActiveTheme;
use gpui_component::ChildElement;
use gpui_component::checkbox::Checkbox;
use gpui_component::input::InputState;
use gpui_component::scroll::ScrollableElement;
use gpui_component::table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow};
use percent_encoding::percent_decode_str;

use crate::document::handler::{FileHandler, FileHandlerEvent};
use crate::document::markdown::{ListItem, MarkdownBlock, MarkdownInline, parse_markdown};
use crate::ui::BASE_FONT_SIZE;

use constants::{
    MD_BLOCKQUOTE_BORDER, MD_BLOCKQUOTE_PADDING, MD_CODE_BLOCK_PADDING, MD_CODE_BLOCK_RADIUS,
    MD_CODE_FONT_SCALE, MD_HEADING_MARGIN, MD_HEADING_SIZES, MD_LINE_HEIGHT, MD_LIST_INDENT,
    MD_PARAGRAPH_MARGIN,
};
use frontmatter::render_frontmatter;

pub struct MarkdownViewer {
    input: Entity<InputState>,
    file_path: PathBuf,
}

#[derive(Clone, Copy, Default)]
struct InlineStyle {
    bold: bool,
    italic: bool,
    code: bool,
}

struct BlockContext<'a> {
    element_id: &'a mut usize,
    handler: Entity<FileHandler>,
    cx: &'a mut App,
}

impl MarkdownViewer {
    pub const fn new(input: Entity<InputState>, file_path: PathBuf) -> Self {
        Self { input, file_path }
    }

    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }

    fn render_image(&self, url: &str, alt: &str, grouped: bool, cx: &App) -> AnyElement {
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
                .map_or_else(|| PathBuf::from(&decoded), |parent| parent.join(&decoded));
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
            .text_size(px(BASE_FONT_SIZE * 0.9))
            .child(if alt.is_empty() {
                format!("[image: {url}]")
            } else {
                format!("[{alt}]")
            })
            .into_any_element()
    }

    pub fn render(
        &self,
        handler: Entity<FileHandler>,
        _window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let content = self.input.read(cx).value().to_string();
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
                BASE_FONT_SIZE,
                &handler,
                cx,
            ));
        }
        let mut ctx = BlockContext {
            element_id: &mut element_id,
            handler,
            cx,
        };
        elements.extend(self.render_blocks(&document.blocks, 0, &mut ctx));

        div()
            .id("markdown-preview")
            .size_full()
            .overflow_y_scroll()
            .overflow_x_hidden()
            .p_4()
            .whitespace_normal()
            .line_height(px(BASE_FONT_SIZE * MD_LINE_HEIGHT))
            .child(div().w_full().min_w_0().children(elements))
            .into_any_element()
    }

    fn render_blocks(
        &self,
        blocks: &[MarkdownBlock],
        list_depth: usize,
        ctx: &mut BlockContext,
    ) -> Vec<AnyElement> {
        blocks
            .iter()
            .enumerate()
            .map(|(index, block)| {
                let next_index = index.saturating_add(1);
                let is_last = next_index == blocks.len();
                let next_is_paragraph =
                    matches!(blocks.get(next_index), Some(MarkdownBlock::Paragraph(_)));
                self.render_block(block, list_depth, is_last, next_is_paragraph, ctx)
            })
            .collect()
    }

    fn render_block(
        &self,
        block: &MarkdownBlock,
        list_depth: usize,
        is_last: bool,
        next_is_paragraph: bool,
        ctx: &mut BlockContext,
    ) -> AnyElement {
        match block {
            MarkdownBlock::Heading { level, content } => self.render_heading(*level, content, ctx),
            MarkdownBlock::Paragraph(content) => {
                self.render_paragraph(content, list_depth, is_last, next_is_paragraph, ctx)
            }
            MarkdownBlock::BlockQuote(blocks) => self.render_blockquote(blocks, list_depth, ctx),
            MarkdownBlock::List { start, items } => {
                self.render_list(*start, items, list_depth, ctx)
            }
            MarkdownBlock::Table { headers, rows } => {
                self.render_table(headers, rows, list_depth, ctx)
            }
            MarkdownBlock::Code { language, content } => {
                Self::render_code(language.as_ref(), content, ctx)
            }
            MarkdownBlock::Rule => div()
                .w_full()
                .my_2()
                .border_t_1()
                .border_color(ctx.cx.theme().border)
                .into_any_element(),
        }
    }

    fn render_heading(
        &self,
        level: u32,
        content: &[MarkdownInline],
        ctx: &mut BlockContext,
    ) -> AnyElement {
        let size = MD_HEADING_SIZES
            .get(
                usize::try_from(level)
                    .unwrap_or_default()
                    .saturating_sub(1)
                    .min(5),
            )
            .copied()
            .unwrap_or_else(|| MD_HEADING_SIZES.last().copied().unwrap_or(1.0));
        div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_wrap()
            .text_size(px(BASE_FONT_SIZE * size))
            .font_weight(FontWeight::BOLD)
            .mt(px(MD_HEADING_MARGIN * size))
            .mb(px(MD_HEADING_MARGIN * size))
            .line_height(px(BASE_FONT_SIZE * size * MD_LINE_HEIGHT))
            .children(self.render_inlines(content, InlineStyle::default(), ctx))
            .into_any_element()
    }

    fn render_paragraph(
        &self,
        content: &[MarkdownInline],
        list_depth: usize,
        is_last: bool,
        next_is_paragraph: bool,
        ctx: &mut BlockContext,
    ) -> AnyElement {
        div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_wrap()
            .mb(px(if is_last || (list_depth > 0 && !next_is_paragraph) {
                0.0
            } else {
                MD_PARAGRAPH_MARGIN
            }))
            .children(self.render_inlines(content, InlineStyle::default(), ctx))
            .into_any_element()
    }

    fn render_blockquote(
        &self,
        blocks: &[MarkdownBlock],
        list_depth: usize,
        ctx: &mut BlockContext,
    ) -> AnyElement {
        div()
            .w_full()
            .min_w_0()
            .pl(px(MD_BLOCKQUOTE_PADDING))
            .border_l(px(MD_BLOCKQUOTE_BORDER))
            .border_color(ctx.cx.theme().border)
            .text_color(ctx.cx.theme().muted_foreground)
            .children(self.render_blocks(blocks, list_depth, ctx))
            .into_any_element()
    }

    fn render_list(
        &self,
        start: Option<u64>,
        items: &[ListItem],
        list_depth: usize,
        ctx: &mut BlockContext,
    ) -> AnyElement {
        let ordered = start.is_some();
        let first = start.unwrap_or(1);
        let rows = items.iter().enumerate().map(|(index, item)| {
            let marker = item.task.map_or_else(
                || {
                    if ordered {
                        div()
                            .child(format!(
                                "{}. ",
                                first.saturating_add(u64::try_from(index).unwrap_or_default())
                            ))
                            .into_any_element()
                    } else {
                        div().child("\u{2022} ".to_string()).into_any_element()
                    }
                },
                |checked| {
                    let id = *ctx.element_id;
                    *ctx.element_id = id.saturating_add(1);
                    div()
                        .flex()
                        .items_center()
                        .flex_shrink_0()
                        .h(px(BASE_FONT_SIZE * MD_LINE_HEIGHT))
                        .mr_1()
                        .child(
                            Checkbox::new(ElementId::NamedInteger(
                                "md-task-check".into(),
                                u64::try_from(id).unwrap_or_default(),
                            ))
                            .checked(checked),
                        )
                        .into_any_element()
                },
            );
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
                    list_depth.saturating_add(1),
                    ctx,
                )))
        });
        div()
            .w_full()
            .children(rows)
            .mb(px(if list_depth > 0 {
                0.0
            } else {
                MD_PARAGRAPH_MARGIN
            }))
            .into_any_element()
    }

    fn render_table(
        &self,
        headers: &[Vec<MarkdownInline>],
        rows: &[Vec<Vec<MarkdownInline>>],
        list_depth: usize,
        ctx: &mut BlockContext,
    ) -> AnyElement {
        let header_style = InlineStyle {
            bold: true,
            ..InlineStyle::default()
        };
        let header_row =
            TableRow::new().children(headers.iter().map(|cell| {
                TableHead::new().children(self.render_inlines(cell, header_style, ctx))
            }));
        let body = TableBody::new().children(rows.iter().map(|row| {
            TableRow::new().children(row.iter().map(|cell| {
                TableCell::new().children(self.render_inlines(cell, InlineStyle::default(), ctx))
            }))
        }));
        let table_ix = *ctx.element_id;
        *ctx.element_id = table_ix.saturating_add(1);
        Table::new()
            .with_ix(table_ix)
            .border_1()
            .border_color(ctx.cx.theme().border)
            .rounded(px(MD_CODE_BLOCK_RADIUS))
            .mb(px(if list_depth > 0 {
                0.0
            } else {
                MD_PARAGRAPH_MARGIN
            }))
            .child(TableHeader::new().child(header_row))
            .child(body)
            .into_any_element()
    }

    fn render_code(language: Option<&String>, content: &str, ctx: &mut BlockContext) -> AnyElement {
        let label = language.map(|language| {
            div()
                .text_size(px(BASE_FONT_SIZE * MD_CODE_FONT_SCALE * 0.8))
                .text_color(ctx.cx.theme().muted_foreground)
                .child(language.clone())
        });
        div()
            .bg(ctx.cx.theme().muted)
            .font_family("monospace")
            .text_size(px(BASE_FONT_SIZE * MD_CODE_FONT_SCALE))
            .rounded(px(MD_CODE_BLOCK_RADIUS))
            .p(px(MD_CODE_BLOCK_PADDING))
            .mb_2()
            .overflow_x_scrollbar()
            .children(label)
            .child(content.to_owned())
            .into_any_element()
    }

    fn render_inlines(
        &self,
        inlines: &[MarkdownInline],
        style: InlineStyle,
        ctx: &mut BlockContext,
    ) -> Vec<AnyElement> {
        let mut elements = Vec::new();
        let mut text = String::new();
        let mut highlights: Vec<(Range<usize>, HighlightStyle)> = Vec::new();
        let mut index = 0;

        while index < inlines.len() {
            if let Some(end) = adjacent_image_run_end(inlines, index) {
                flush_inline_text(&mut elements, &mut text, &mut highlights);
                if let Some(run) = inlines.get(index..end) {
                    let images = run.iter().filter_map(|inline| {
                        let MarkdownInline::Image { url, alt } = inline else {
                            return None;
                        };
                        Some(self.render_image(url, alt, true, ctx.cx))
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
                }
                index = end;
                continue;
            }

            let Some(inline) = inlines.get(index) else {
                break;
            };
            match inline {
                MarkdownInline::Link { url, content } => {
                    flush_inline_text(&mut elements, &mut text, &mut highlights);
                    let link_url = url.clone();
                    let handler_clone = ctx.handler.clone();
                    elements.push(
                        div()
                            .id(SharedString::from(format!("link-{url}")))
                            .flex()
                            .text_color(ctx.cx.theme().primary)
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
                            .children(self.render_inlines(content, style, ctx))
                            .into_any_element(),
                    );
                }
                MarkdownInline::Image { url, alt } => {
                    flush_inline_text(&mut elements, &mut text, &mut highlights);
                    elements.push(self.render_image(url, alt, false, ctx.cx));
                }
                MarkdownInline::Break => {
                    flush_inline_text(&mut elements, &mut text, &mut highlights);
                    elements.push(div().w_full().into_any_element());
                }
                _ => {
                    Self::append_inline_text(
                        std::slice::from_ref(inline),
                        style,
                        ctx.cx,
                        &mut text,
                        &mut highlights,
                    );
                }
            }
            index = index.saturating_add(1);
        }
        flush_inline_text(&mut elements, &mut text, &mut highlights);
        elements
    }

    fn append_inline_text(
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
                        inline_highlight(
                            InlineStyle {
                                code: true,
                                ..style
                            },
                            cx,
                        ),
                    ));
                }
                MarkdownInline::Strong(children) => Self::append_inline_text(
                    children,
                    InlineStyle {
                        bold: true,
                        ..style
                    },
                    cx,
                    text,
                    highlights,
                ),
                MarkdownInline::Emphasis(children) => Self::append_inline_text(
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

fn flush_inline_text(
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

fn adjacent_image_run_end(inlines: &[MarkdownInline], start: usize) -> Option<usize> {
    if !matches!(inlines.get(start), Some(MarkdownInline::Image { .. })) {
        return None;
    }

    let mut index = start.saturating_add(1);
    let mut image_count = 1usize;
    while index < inlines.len() {
        match inlines.get(index) {
            Some(MarkdownInline::Image { .. }) => {
                image_count = image_count.saturating_add(1);
                index = index.saturating_add(1);
            }
            Some(MarkdownInline::Text(text)) if text.chars().all(char::is_whitespace) => {
                index = index.saturating_add(1);
            }
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
