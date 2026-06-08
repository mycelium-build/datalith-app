use std::cell::RefCell;
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;

use gpui::*;
use gpui_component::ActiveTheme;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::scroll::ScrollableElement;
use percent_encoding::percent_decode_str;

use crate::consts::{
    BASE_FONT_SIZE, MD_BLOCKQUOTE_BORDER, MD_BLOCKQUOTE_PADDING, MD_CODE_BLOCK_PADDING,
    MD_CODE_BLOCK_RADIUS, MD_CODE_FONT_SCALE, MD_FRONTMATTER_FONT_SCALE, MD_FRONTMATTER_MARGIN,
    MD_FRONTMATTER_PADDING, MD_FRONTMATTER_RADIUS, MD_HEADING_MARGIN, MD_HEADING_SIZES,
    MD_IMAGE_MAX_WIDTH, MD_LINE_HEIGHT, MD_LIST_INDENT,
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

        let mut text_buffer = TextBuffer::new();

        for event in events {
            match event {
                MarkdownEvent::Text(text, style) => {
                    if let Some(ref url) = current_link_url {
                        text_buffer.push_link(&text, url, &style, cx);
                    } else {
                        text_buffer.push(&text, &style, cx);
                    }
                }
                MarkdownEvent::LinkStart(url) => {
                    current_link_url = Some(url);
                }
                MarkdownEvent::LinkEnd => {
                    current_link_url = None;
                }
                MarkdownEvent::BlockStart(block) => {
                    flush_inline(&mut text_buffer, &mut current_list_item, &mut current_line, cx);
                    if !matches!(block, MarkdownBlock::Paragraph) {
                        if !current_line.is_empty() {
                            paragraph_lines.push(wrap_line(&mut current_line));
                        }
                        if !paragraph_lines.is_empty() {
                            elements.push(wrap_paragraph(&mut paragraph_lines));
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
                                    .overflow_x_scrollbar()
                                    .child(code)
                                    .into_any_element(),
                            );
                        }
                        MarkdownBlock::Frontmatter(fm_content) => {
                            elements.push(render_frontmatter(&fm_content, base_font_size, cx));
                        }
                        _ => {}
                    }
                }
                MarkdownEvent::BlockEnd => {
                    flush_inline(&mut text_buffer, &mut current_list_item, &mut current_line, cx);
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
                                    elements.push(wrap_paragraph(&mut paragraph_lines));
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
                    flush_inline(&mut text_buffer, &mut current_list_item, &mut current_line, cx);
                    if !current_line.is_empty() {
                        let wrapped = wrap_line(&mut current_line);
                        if in_paragraph {
                            paragraph_lines.push(wrapped);
                        } else {
                            elements.push(wrapped);
                        }
                    }
                    if in_paragraph && !paragraph_lines.is_empty() {
                        elements.push(wrap_paragraph(&mut paragraph_lines));
                        in_paragraph = false;
                    }
                    elements.push(self.render_image(&url, &alt, cx));
                }
            }
        }

        flush_inline(&mut text_buffer, &mut current_list_item, &mut current_line, cx);
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
            elements.push(wrap_paragraph(&mut paragraph_lines));
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

struct TextBuffer {
    text: String,
    highlights: Vec<(Range<usize>, HighlightStyle)>,
    links: Vec<(Range<usize>, SharedString)>,
    layout: Rc<RefCell<Option<TextLayout>>>,
    heading_level: Option<u32>,
}

struct FlushedText {
    text: String,
    highlights: Vec<(Range<usize>, HighlightStyle)>,
    links: Vec<(Range<usize>, SharedString)>,
    layout: Rc<RefCell<Option<TextLayout>>>,
    heading_level: Option<u32>,
}

impl TextBuffer {
    fn new() -> Self {
        Self {
            text: String::new(),
            highlights: Vec::new(),
            links: Vec::new(),
            layout: Rc::new(RefCell::new(None)),
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

    fn push_link(&mut self, text: &str, url: &str, style: &MarkdownStyle, cx: &App) {
        if text.is_empty() {
            return;
        }
        let start = self.text.len();
        self.text.push_str(text);
        let end = self.text.len();
        let base_highlight = build_highlight_style(style, cx);
        let link = link_highlight_style(cx);
        let combined = HighlightStyle {
            color: link.color,
            underline: link.underline,
            ..base_highlight
        };
        self.highlights.push((start..end, combined));
        self.links
            .push((start..end, SharedString::from(url.to_string())));
    }

    fn flush(&mut self) -> Option<FlushedText> {
        if self.text.is_empty() {
            return None;
        }
        Some(FlushedText {
            text: std::mem::take(&mut self.text),
            highlights: std::mem::take(&mut self.highlights),
            links: std::mem::take(&mut self.links),
            layout: self.layout.clone(),
            heading_level: self.heading_level.take(),
        })
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

fn render_frontmatter(content: &str, base_font_size: f32, cx: &App) -> AnyElement {
    let lines: Vec<&str> = content.lines().collect();
    let mut max_key_len = 0;
    for line in &lines {
        if let Some(colon_pos) = line.find(':') {
            max_key_len = max_key_len.max(colon_pos);
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

fn wrap_paragraph(paragraph_lines: &mut Vec<AnyElement>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .w_full()
        .min_w_0()
        .children(paragraph_lines.drain(..))
        .into_any_element()
}

fn flush_inline(
    text_buffer: &mut TextBuffer,
    current_list_item: &mut Option<ListItemData>,
    current_line: &mut Vec<AnyElement>,
    cx: &mut Context<MarkdownEditor>,
) {
    if let Some(element) = flush_text_buffer(text_buffer, cx) {
        if let Some(li) = current_list_item {
            li.elements.push(element);
        } else {
            current_line.push(element);
        }
    }
}

fn flush_text_buffer(
    text_buffer: &mut TextBuffer,
    cx: &mut Context<MarkdownEditor>,
) -> Option<AnyElement> {
    let data = text_buffer.flush()?;
    let layout = data.layout.clone();
    let links = data.links;
    let heading_level = data.heading_level;
    let inline_text = InlineText {
        text: SharedString::from(data.text),
        highlights: data.highlights,
        layout: data.layout,
    };
    let mut el = div().w_full().id("inline-text");
    if let Some(level) = heading_level {
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
    let div = el
        .child(inline_text)
        .on_click(cx.listener(move |_, event: &ClickEvent, _, cx| {
            let layout_ref = layout.borrow();
            if let Some(ref text_layout) = *layout_ref {
                let position = event.position();
                let index = text_layout
                    .index_for_position(position)
                    .unwrap_or_else(|e| e);
                for (range, url) in links.iter() {
                    if range.contains(&index) {
                        cx.emit(MarkdownEditorEvent::LinkClicked(
                            url.to_string(),
                            event.modifiers().platform,
                        ));
                        return;
                    }
                }
            }
        }));
    Some(div.into_any_element())
}

struct InlineText {
    text: SharedString,
    highlights: Vec<(Range<usize>, HighlightStyle)>,
    layout: Rc<RefCell<Option<TextLayout>>>,
}

impl Element for InlineText {
    type RequestLayoutState = StyledText;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut styled =
            StyledText::new(self.text.clone()).with_highlights(self.highlights.clone());
        let (layout_id, _) = styled.request_layout(id, inspector_id, window, cx);
        *self.layout.borrow_mut() = Some(styled.layout().clone());
        (layout_id, styled)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        styled: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let _ = styled.prepaint(id, inspector_id, bounds, &mut (), window, cx);
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        styled: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        styled.paint(id, inspector_id, bounds, &mut (), prepaint, window, cx)
    }
}

impl IntoElement for InlineText {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}
