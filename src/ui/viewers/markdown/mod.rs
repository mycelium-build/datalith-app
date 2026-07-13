mod constants;
mod frontmatter;

use std::ops::Range;
use std::path::PathBuf;

use gpui::*;
use gpui_component::ActiveTheme;
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
                    .my_2()
                    .child(img(path).max_w(relative(1.)))
                    .into_any_element();
            }
        }

        div()
            .w_full()
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

        let document = parse_markdown(&content);
        let mut elements = Vec::new();
        if let Some(frontmatter) = &document.frontmatter {
            elements.push(render_frontmatter(
                frontmatter,
                base_font_size,
                handler.clone(),
                cx,
            ));
        }
        elements.extend(self.render_blocks(&document.blocks, 0, handler, cx));

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
        handler: Entity<FileHandler>,
        cx: &mut App,
    ) -> Vec<AnyElement> {
        blocks
            .iter()
            .map(|block| self.render_block(block, list_depth, handler.clone(), cx))
            .collect()
    }

    fn render_block(
        &self,
        block: &MarkdownBlock,
        list_depth: usize,
        handler: Entity<FileHandler>,
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
                .mb_2()
                .children(self.render_inlines(content, InlineStyle::default(), handler, cx))
                .into_any_element(),
            MarkdownBlock::BlockQuote(blocks) => div()
                .w_full()
                .min_w_0()
                .pl(px(MD_BLOCKQUOTE_PADDING))
                .border_l(px(MD_BLOCKQUOTE_BORDER))
                .border_color(cx.theme().border)
                .text_color(cx.theme().muted_foreground)
                .children(self.render_blocks(blocks, list_depth, handler, cx))
                .into_any_element(),
            MarkdownBlock::List { start, items } => {
                let ordered = start.is_some();
                let first = start.unwrap_or(1);
                let rows = items.iter().enumerate().map(|(index, blocks)| {
                    let marker = if ordered {
                        format!("{}. ", first + index as u64)
                    } else {
                        "\u{2022} ".to_string()
                    };
                    div()
                        .flex()
                        .items_start()
                        .w_full()
                        .min_w_0()
                        .child(div().flex_shrink_0().child(format!(
                            "{}{}",
                            MD_LIST_INDENT.repeat(list_depth),
                            marker
                        )))
                        .child(div().flex_1().min_w_0().children(self.render_blocks(
                            blocks,
                            list_depth + 1,
                            handler.clone(),
                            cx,
                        )))
                });
                div().w_full().children(rows).mb_2().into_any_element()
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
        inlines
            .iter()
            .map(|inline| match inline {
                MarkdownInline::Text(text) => render_styled_text(text, style, cx),
                MarkdownInline::Code(code) => render_styled_text(
                    code,
                    InlineStyle {
                        code: true,
                        ..style
                    },
                    cx,
                ),
                MarkdownInline::Strong(children) => self.wrap_inlines(
                    children,
                    InlineStyle {
                        bold: true,
                        ..style
                    },
                    handler.clone(),
                    cx,
                ),
                MarkdownInline::Emphasis(children) => self.wrap_inlines(
                    children,
                    InlineStyle {
                        italic: true,
                        ..style
                    },
                    handler.clone(),
                    cx,
                ),
                MarkdownInline::Link { url, content } => {
                    let link_url = url.clone();
                    let handler_clone = handler.clone();
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
                        .into_any_element()
                }
                MarkdownInline::Image { url, alt } => self.render_image(url, alt, cx),
                MarkdownInline::Break => div().w_full().into_any_element(),
            })
            .collect()
    }

    fn wrap_inlines(
        &self,
        inlines: &[MarkdownInline],
        style: InlineStyle,
        handler: Entity<FileHandler>,
        cx: &mut App,
    ) -> AnyElement {
        div()
            .flex()
            .children(self.render_inlines(inlines, style, handler, cx))
            .into_any_element()
    }
}

fn render_styled_text(text: &str, style: InlineStyle, cx: &App) -> AnyElement {
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
    let styled = StyledText::new(SharedString::from(text.to_string())).with_highlights(vec![(
        Range {
            start: 0,
            end: text.len(),
        },
        highlight,
    )]);
    div().min_w_0().child(styled).into_any_element()
}
