use gpui::*;
use gpui_component::ActiveTheme;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::scroll::ScrollableElement;

use crate::consts::{
    BASE_FONT_SIZE, MD_BLOCKQUOTE_BORDER, MD_BLOCKQUOTE_PADDING, MD_CODE_BLOCK_PADDING,
    MD_CODE_BLOCK_RADIUS, MD_CODE_FONT_SCALE, MD_CODE_PADDING, MD_CODE_RADIUS,
    MD_FRONTMATTER_FONT_SCALE, MD_FRONTMATTER_MARGIN, MD_FRONTMATTER_PADDING,
    MD_FRONTMATTER_RADIUS, MD_HEADING_MARGIN, MD_HEADING_SIZES, MD_LINE_HEIGHT, MD_LIST_INDENT,
};
use crate::markdown::{MarkdownBlock, MarkdownEvent, MarkdownStyle, parse_markdown};

pub struct MarkdownEditor {
    input: Entity<InputState>,
    editing: bool,
    _sub: Option<Subscription>,
}

struct ListItemData {
    marker_text: String,
    is_ordered: bool,
    indent: usize,
    text_content: String,
    text_style: MarkdownStyle,
}

impl MarkdownEditor {
    pub fn new(window: &mut Window, cx: &mut Context<Self>, content: String) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .searchable(true)
                .default_value(content)
        });

        let sub = cx.subscribe(&input, |_this, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });

        Self {
            input,
            editing: false,
            _sub: Some(sub),
        }
    }

    pub fn input(&self) -> &Entity<InputState> {
        &self.input
    }

    fn start_editing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editing {
            self.editing = true;
            self.input.focus_handle(cx).focus(window, cx);
            cx.notify();
        }
    }

    fn stop_editing(&mut self, cx: &mut Context<Self>) {
        if self.editing {
            self.editing = false;
            cx.notify();
        }
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

        let flush_list_item = |item: &mut Option<ListItemData>,
                               out: &mut Vec<AnyElement>,
                               _base_fs: f32,
                               cx: &App| {
            if let Some(li) = item.take() {
                let marker = if li.is_ordered {
                    format!("{}. ", li.marker_text)
                } else {
                    "\u{2022} ".to_string()
                };
                let indent_str = MD_LIST_INDENT.repeat(li.indent);
                let full_text = format!("{}{}{}", indent_str, marker, li.text_content);
                let mut span = div().child(full_text);
                span = apply_style_to_div(span, &li.text_style, cx);
                out.push(span.into_any_element());
            }
        };

        for event in events {
            match event {
                MarkdownEvent::Text(text, style) => {
                    if let Some(ref mut li) = current_list_item {
                        li.text_content.push_str(&text);
                        li.text_style = style;
                    } else {
                        let mut span = div().child(text);
                        span = apply_style_to_div(span, &style, cx);
                        elements.push(span.into_any_element());
                    }
                }
                MarkdownEvent::BlockStart(block) => {
                    block_stack.push(block.clone());
                    match block {
                        MarkdownBlock::Heading(_) => {}
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
                                flush_list_item(
                                    &mut current_list_item,
                                    &mut elements,
                                    base_font_size,
                                    cx,
                                );
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
                                text_content: String::new(),
                                text_style: MarkdownStyle::Normal,
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
                            let mut fm_elements: Vec<AnyElement> = Vec::new();
                            fm_elements.push(
                                div()
                                    .text_size(px(base_font_size * MD_FRONTMATTER_FONT_SCALE))
                                    .text_color(cx.theme().muted_foreground)
                                    .font_family("monospace")
                                    .child("---")
                                    .into_any_element(),
                            );
                            for line in lines {
                                if let Some(colon_pos) = line.find(':') {
                                    let key = &line[..colon_pos];
                                    let value = line[colon_pos + 1..].trim();
                                    fm_elements.push(
                                        div()
                                            .flex()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_size(px(base_font_size * MD_FRONTMATTER_FONT_SCALE))
                                                    .text_color(cx.theme().accent)
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .child(key.to_string()),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(base_font_size * MD_FRONTMATTER_FONT_SCALE))
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(value.to_string()),
                                            )
                                            .into_any_element(),
                                    );
                                } else {
                                    fm_elements.push(
                                        div()
                                            .text_size(px(base_font_size * MD_FRONTMATTER_FONT_SCALE))
                                            .text_color(cx.theme().muted_foreground)
                                            .child(line.to_string())
                                            .into_any_element(),
                                    );
                                }
                            }
                            fm_elements.push(
                                div()
                                    .text_size(px(base_font_size * MD_FRONTMATTER_FONT_SCALE))
                                    .text_color(cx.theme().muted_foreground)
                                    .font_family("monospace")
                                    .child("---")
                                    .into_any_element(),
                            );
                            elements.push(
                                div()
                                    .bg(cx.theme().muted.opacity(0.3))
                                    .rounded(px(MD_FRONTMATTER_RADIUS))
                                    .p(px(MD_FRONTMATTER_PADDING))
                                    .mb(px(MD_FRONTMATTER_MARGIN))
                                    .children(fm_elements)
                                    .into_any_element(),
                            );
                        }
                    }
                }
                MarkdownEvent::BlockEnd => {
                    if let Some(popped) = block_stack.pop() {
                        match popped {
                            MarkdownBlock::ListItem(_) => {
                                flush_list_item(
                                    &mut current_list_item,
                                    &mut elements,
                                    base_font_size,
                                    cx,
                                );
                            }
                            MarkdownBlock::List(_, depth) => {
                                if in_ordered_list && depth > 0 && ordered_counters.len() > depth {
                                    ordered_counters.truncate(depth);
                                }
                            }
                            _ => {}
                        }
                    }
                    elements.push(div().mb_1().into_any_element());
                }
            }
        }

        flush_list_item(&mut current_list_item, &mut elements, base_font_size, cx);

        div()
            .id("markdown-preview")
            .size_full()
            .on_click(cx.listener(|this, _, window, cx| {
                this.start_editing(window, cx);
            }))
            .child(
                div()
                    .size_full()
                    .p_4()
                    .overflow_y_scrollbar()
                    .line_height(px(base_font_size * MD_LINE_HEIGHT))
                    .child(div().children(elements)),
            )
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
        div()
            .size_full()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                if event.keystroke.key == "escape" && this.editing {
                    this.stop_editing(cx);
                }
            }))
            .child(if self.editing {
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
            el = el
                .text_size(px(base_font_size * size))
                .font_weight(FontWeight::BOLD)
                .mb(px(MD_HEADING_MARGIN));
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
            el = el.text_color(cx.theme().accent).underline();
        }
        MarkdownStyle::Normal => {}
    }

    el
}
