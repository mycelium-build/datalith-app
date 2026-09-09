//! Block-level Markdown rendering: dispatch, blockquotes, lists, tables, code.

use std::ops::Range;

use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::{
    ActiveTheme, ChildElement,
    checkbox::Checkbox,
    table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow},
};
use gpui_kit::{
    AnyElement, App, ElementId, FontStyle, FontWeight, HighlightStyle, IntoElement, ParentElement,
    SharedString, Styled, StyledText, div, px,
};

use super::constants::{
    MD_BLOCKQUOTE_BORDER, MD_BLOCKQUOTE_PADDING, MD_CODE_BLOCK_PADDING, MD_CODE_BLOCK_RADIUS,
    MD_CODE_FONT_SCALE, MD_LINE_HEIGHT, MD_LIST_INDENT, MD_PARAGRAPH_MARGIN,
};
use super::{BlockContext, InlineStyle, MarkdownViewer};
use crate::document::markdown::{ListItem, MarkdownBlock, MarkdownInline};
use crate::ui::BASE_FONT_SIZE;

impl MarkdownViewer {
    pub(super) fn render_blocks(
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

    pub(super) fn render_list(
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

    pub(super) fn render_table(
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

    pub(super) fn render_code(
        language: Option<&String>,
        content: &str,
        ctx: &mut BlockContext,
    ) -> AnyElement {
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
}

pub(super) fn flush_inline_text(
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

pub(super) fn adjacent_image_run_end(inlines: &[MarkdownInline], start: usize) -> Option<usize> {
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

pub(super) fn inline_highlight(style: InlineStyle, cx: &App) -> HighlightStyle {
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
    use super::adjacent_image_run_end;
    use crate::document::markdown::{MarkdownInline, MarkdownInline as MI};

    fn image(name: &str) -> MarkdownInline {
        MarkdownInline::Image {
            url: name.into(),
            alt: String::new(),
        }
    }

    #[test]
    fn adjacent_images_form_a_run_across_inline_whitespace() {
        let inlines = [image("a.png"), MI::Text(" ".into()), image("b.png")];

        assert_eq!(adjacent_image_run_end(&inlines, 0), Some(3));
    }

    #[test]
    fn a_line_break_prevents_an_image_run() {
        let inlines = [image("a.png"), MI::Break, image("b.png")];

        assert_eq!(adjacent_image_run_end(&inlines, 0), None);
    }

    #[test]
    fn intervening_text_prevents_an_image_run() {
        let inlines = [
            image("a.png"),
            MI::Text(" some text ".into()),
            image("b.png"),
        ];

        assert_eq!(adjacent_image_run_end(&inlines, 0), None);
    }
}
