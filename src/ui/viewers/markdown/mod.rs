mod blocks;

use blocks::{adjacent_image_run_end, flush_inline_text, inline_highlight};
mod constants;
mod frontmatter;

use std::ops::Range;
use std::path::PathBuf;

use gpui_kit::component::ActiveTheme;
use gpui_kit::component::input::EditorState;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::{
    AnyElement, App, ClickEvent, Entity, FocusHandle, Focusable, FontWeight, HighlightStyle,
    InteractiveElement, IntoElement, ParentElement, SharedString, SharedUri,
    StatefulInteractiveElement, Styled, div, img, px, relative,
};
use percent_encoding::percent_decode_str;

use crate::app::fonts::PIXELOID_FONT;
use crate::document::handler::{FileHandler, FileHandlerEvent};
use crate::document::markdown::{MarkdownInline, parse_markdown};
use crate::ui::BASE_FONT_SIZE;
use crate::vault::path::display_name;

use constants::{
    MD_HEADING_MARGIN, MD_HEADING_SIZES, MD_LINE_HEIGHT, MD_PARAGRAPH_MARGIN, MD_TITLE_SIZE,
};
use frontmatter::render_frontmatter;

pub struct MarkdownViewer {
    input: Entity<EditorState>,
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
    pub const fn new(input: Entity<EditorState>, file_path: PathBuf) -> Self {
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
        if url.starts_with("http://") || url.starts_with("https://") {
            return container
                .child(img(SharedUri::from(url.to_string())).max_w(relative(1.)))
                .into_any_element();
        }
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

    pub fn render(&self, handler: Entity<FileHandler>, cx: &mut App) -> AnyElement {
        let content = self.input.read(cx).value().to_string();
        if content.is_empty() {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(cx.theme().muted_foreground)
                .child("Empty markdown")
                .into_any_element();
        }

        let document = parse_markdown(&content);
        let mut elements = Vec::new();
        let mut element_id = 0usize;
        elements.push(self.render_title());
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
            .min_h_0()
            .overflow_y_scrollbar()
            .overflow_x_hidden()
            .p_4()
            .whitespace_normal()
            .line_height(px(BASE_FONT_SIZE * MD_LINE_HEIGHT))
            .child(div().w_full().min_w_0().children(elements))
            .into_any_element()
    }

    fn render_title(&self) -> AnyElement {
        let name = self
            .file_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map_or_else(|| display_name(&self.file_path).to_owned(), str::to_owned);
        div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_wrap()
            .font_family(PIXELOID_FONT)
            .font_weight(FontWeight::BOLD)
            .text_size(px(BASE_FONT_SIZE * MD_TITLE_SIZE))
            .line_height(px(BASE_FONT_SIZE * MD_TITLE_SIZE * MD_LINE_HEIGHT))
            .mb(px(MD_HEADING_MARGIN * MD_TITLE_SIZE))
            .child(name)
            .into_any_element()
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
            .font_family(PIXELOID_FONT)
            .text_size(px(BASE_FONT_SIZE * size))
            //.font_weight(FontWeight::BOLD)
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
                                        event.modifiers().secondary(),
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
                    if Self::try_append_inline_text(
                        inline,
                        style,
                        ctx.cx,
                        &mut text,
                        &mut highlights,
                    ) {
                        // consumed as styled text
                    } else if let Some(children) = Self::strong_or_emphasis_children(inline) {
                        // Strong/Emphasis wrapping a link, image, or break
                        // cannot be flattened into the styled text;
                        // render its children separately.
                        flush_inline_text(&mut elements, &mut text, &mut highlights);
                        let child_style = if matches!(inline, MarkdownInline::Strong(_)) {
                            InlineStyle {
                                bold: true,
                                ..style
                            }
                        } else {
                            InlineStyle {
                                italic: true,
                                ..style
                            }
                        };
                        elements.extend(self.render_inlines(children, child_style, ctx));
                    }
                }
            }
            index = index.saturating_add(1);
        }
        flush_inline_text(&mut elements, &mut text, &mut highlights);
        elements
    }

    fn strong_or_emphasis_children(inline: &MarkdownInline) -> Option<&[MarkdownInline]> {
        match inline {
            MarkdownInline::Strong(children) | MarkdownInline::Emphasis(children) => Some(children),
            _ => None,
        }
    }

    fn is_textable(inline: &MarkdownInline) -> bool {
        match inline {
            MarkdownInline::Text(_) | MarkdownInline::Code(_) => true,
            MarkdownInline::Strong(children) | MarkdownInline::Emphasis(children) => {
                children.iter().all(Self::is_textable)
            }
            _ => false,
        }
    }

    fn try_append_inline_text(
        inline: &MarkdownInline,
        style: InlineStyle,
        cx: &App,
        text: &mut String,
        highlights: &mut Vec<(Range<usize>, HighlightStyle)>,
    ) -> bool {
        match inline {
            MarkdownInline::Text(value) => {
                let start = text.len();
                text.push_str(value);
                highlights.push((start..text.len(), inline_highlight(style, cx)));
                true
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
                true
            }
            MarkdownInline::Strong(children) | MarkdownInline::Emphasis(children) => {
                if !children.iter().all(Self::is_textable) {
                    return false;
                }
                let child_style = if matches!(inline, MarkdownInline::Strong(_)) {
                    InlineStyle {
                        bold: true,
                        ..style
                    }
                } else {
                    InlineStyle {
                        italic: true,
                        ..style
                    }
                };
                for child in children {
                    Self::try_append_inline_text(child, child_style, cx, text, highlights);
                }
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MarkdownViewer;
    use crate::document::markdown::MarkdownInline;

    #[test]
    fn strong_or_emphasis_wrapping_a_link_is_not_flattened_into_text() {
        let link = MarkdownInline::Link {
            url: "a.md".into(),
            content: vec![MarkdownInline::Text("a".into())],
        };
        let strong_with_link =
            MarkdownInline::Strong(vec![MarkdownInline::Text("text ".into()), link]);
        assert!(!MarkdownViewer::is_textable(&strong_with_link));
        assert!(MarkdownViewer::is_textable(&MarkdownInline::Strong(vec![
            MarkdownInline::Text("only text".into())
        ])));
        assert!(MarkdownViewer::is_textable(&MarkdownInline::Emphasis(
            vec![MarkdownInline::Code("c".into())]
        )));
        assert!(!MarkdownViewer::is_textable(&MarkdownInline::Break));
    }
}
