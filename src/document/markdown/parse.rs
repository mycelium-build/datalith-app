use std::iter::Peekable;

use pulldown_cmark::{Event, HeadingLevel, Tag, TagEnd};

use super::{ListItem, MarkdownBlock, MarkdownInline};

pub(super) fn parse_blocks<'a, I>(
    events: &mut Peekable<I>,
    stop: Option<TagEnd>,
) -> Vec<MarkdownBlock>
where
    I: Iterator<Item = Event<'a>>,
{
    let mut blocks = Vec::new();
    while let Some(event) = events.next() {
        if let Event::End(end) = &event
            && stop.as_ref() == Some(end)
        {
            break;
        }
        match event {
            Event::Start(Tag::Heading { level, .. }) => blocks.push(MarkdownBlock::Heading {
                level: heading_level(level),
                content: parse_inlines(events, TagEnd::Heading(level)),
            }),
            Event::Start(Tag::Paragraph) => blocks.push(MarkdownBlock::Paragraph(parse_inlines(
                events,
                TagEnd::Paragraph,
            ))),
            Event::Start(Tag::BlockQuote(kind)) => blocks.push(MarkdownBlock::BlockQuote(
                parse_blocks(events, Some(TagEnd::BlockQuote(kind))),
            )),
            Event::Start(Tag::List(start)) => {
                let mut items = Vec::new();
                while let Some(next) = events.next() {
                    match next {
                        Event::Start(Tag::Item) => {
                            let task = match events.peek() {
                                Some(Event::TaskListMarker(checked)) => Some(*checked),
                                _ => None,
                            };
                            if task.is_some() {
                                events.next();
                            }
                            items.push(ListItem {
                                task,
                                blocks: parse_blocks(events, Some(TagEnd::Item)),
                            });
                        }
                        Event::End(TagEnd::List(_)) => break,
                        _ => {}
                    }
                }
                blocks.push(MarkdownBlock::List { start, items });
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                let language = match kind {
                    pulldown_cmark::CodeBlockKind::Indented => None,
                    pulldown_cmark::CodeBlockKind::Fenced(language) if language.is_empty() => None,
                    pulldown_cmark::CodeBlockKind::Fenced(language) => Some(language.to_string()),
                };
                let mut content = String::new();
                for next in events.by_ref() {
                    match next {
                        Event::End(TagEnd::CodeBlock) => break,
                        Event::Text(text) | Event::Code(text) => content.push_str(&text),
                        Event::SoftBreak | Event::HardBreak => content.push('\n'),
                        _ => {}
                    }
                }
                if content.ends_with('\n') {
                    content.pop();
                }
                blocks.push(MarkdownBlock::Code { language, content });
            }
            Event::Rule => blocks.push(MarkdownBlock::Rule),
            Event::Start(Tag::Table(_)) => {
                let mut headers = Vec::new();
                let mut rows = Vec::new();
                while let Some(event) = events.next() {
                    match event {
                        Event::Start(Tag::TableHead) => {
                            while let Some(inner) = events.next() {
                                match inner {
                                    Event::Start(Tag::TableCell) => {
                                        headers
                                            .push(parse_inlines(events, TagEnd::TableCell));
                                    }
                                    Event::End(TagEnd::TableHead) => break,
                                    _ => {}
                                }
                            }
                        }
                        Event::Start(Tag::TableRow) => rows.push(parse_table_cells(events)),
                        Event::End(TagEnd::Table) => break,
                        _ => {}
                    }
                }
                blocks.push(MarkdownBlock::Table { headers, rows });
            }
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => {
                append_inline(&mut blocks, MarkdownInline::Text(text.to_string()))
            }
            Event::Code(code) => append_inline(&mut blocks, MarkdownInline::Code(code.to_string())),
            Event::SoftBreak | Event::HardBreak => {
                append_inline(&mut blocks, MarkdownInline::Break)
            }
            Event::Start(Tag::Strong) => append_inline(
                &mut blocks,
                MarkdownInline::Strong(parse_inlines(events, TagEnd::Strong)),
            ),
            Event::Start(Tag::Emphasis) => append_inline(
                &mut blocks,
                MarkdownInline::Emphasis(parse_inlines(events, TagEnd::Emphasis)),
            ),
            Event::Start(Tag::Link { dest_url, .. }) => {
                let content = parse_inlines(events, TagEnd::Link);
                append_inline(
                    &mut blocks,
                    MarkdownInline::Link {
                        url: dest_url.to_string(),
                        content,
                    },
                );
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                let content = parse_inlines(events, TagEnd::Image);
                append_inline(
                    &mut blocks,
                    MarkdownInline::Image {
                        url: dest_url.to_string(),
                        alt: inline_plain_text(&content),
                    },
                );
            }
            Event::Start(tag) => {
                let end = tag.to_end();
                let content = parse_inlines(events, end);
                if !content.is_empty() {
                    blocks.push(MarkdownBlock::Paragraph(content));
                }
            }
            _ => {}
        }
    }
    blocks
}

fn append_inline(blocks: &mut Vec<MarkdownBlock>, inline: MarkdownInline) {
    if let Some(MarkdownBlock::Paragraph(content)) = blocks.last_mut() {
        content.push(inline);
    } else {
        blocks.push(MarkdownBlock::Paragraph(vec![inline]));
    }
}

fn parse_table_cells<'a, I>(events: &mut Peekable<I>) -> Vec<Vec<MarkdownInline>>
where
    I: Iterator<Item = Event<'a>>,
{
    let mut cells = Vec::new();
    while let Some(event) = events.next() {
        match event {
            Event::Start(Tag::TableCell) => cells.push(parse_inlines(events, TagEnd::TableCell)),
            Event::End(TagEnd::TableRow) => break,
            _ => {}
        }
    }
    cells
}

fn parse_inlines<'a, I>(events: &mut Peekable<I>, stop: TagEnd) -> Vec<MarkdownInline>
where
    I: Iterator<Item = Event<'a>>,
{
    let mut inlines = Vec::new();
    while let Some(event) = events.next() {
        match event {
            Event::End(end) if end == stop => break,
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => {
                inlines.push(MarkdownInline::Text(text.to_string()))
            }
            Event::Code(code) => inlines.push(MarkdownInline::Code(code.to_string())),
            Event::SoftBreak | Event::HardBreak => inlines.push(MarkdownInline::Break),
            Event::Start(Tag::Strong) => inlines.push(MarkdownInline::Strong(parse_inlines(
                events,
                TagEnd::Strong,
            ))),
            Event::Start(Tag::Emphasis) => inlines.push(MarkdownInline::Emphasis(parse_inlines(
                events,
                TagEnd::Emphasis,
            ))),
            Event::Start(Tag::Link { dest_url, .. }) => inlines.push(MarkdownInline::Link {
                url: dest_url.to_string(),
                content: parse_inlines(events, TagEnd::Link),
            }),
            Event::Start(Tag::Image { dest_url, .. }) => {
                let content = parse_inlines(events, TagEnd::Image);
                inlines.push(MarkdownInline::Image {
                    url: dest_url.to_string(),
                    alt: inline_plain_text(&content),
                });
            }
            Event::Start(tag) => {
                let end = tag.to_end();
                inlines.extend(parse_inlines(events, end));
            }
            _ => {}
        }
    }
    inlines
}

fn inline_plain_text(inlines: &[MarkdownInline]) -> String {
    let mut text = String::new();
    for inline in inlines {
        match inline {
            MarkdownInline::Text(value) | MarkdownInline::Code(value) => text.push_str(value),
            MarkdownInline::Strong(children) | MarkdownInline::Emphasis(children) => {
                text.push_str(&inline_plain_text(children))
            }
            MarkdownInline::Link { content, .. } => text.push_str(&inline_plain_text(content)),
            MarkdownInline::Image { alt, .. } => text.push_str(alt),
            MarkdownInline::Break => text.push('\n'),
        }
    }
    text
}

fn heading_level(level: HeadingLevel) -> u32 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}
