use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use std::iter::Peekable;

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

const ENCODE_IN_LINK: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'<').add(b'>').add(b'`');

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MarkdownDocument {
    pub(crate) frontmatter: Option<Frontmatter>,
    pub(crate) blocks: Vec<MarkdownBlock>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MarkdownBlock {
    Heading {
        level: u32,
        content: Vec<MarkdownInline>,
    },
    Paragraph(Vec<MarkdownInline>),
    List {
        start: Option<u64>,
        items: Vec<Vec<MarkdownBlock>>,
    },
    BlockQuote(Vec<MarkdownBlock>),
    Code {
        language: Option<String>,
        content: String,
    },
    Rule,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MarkdownInline {
    Text(String),
    Strong(Vec<MarkdownInline>),
    Emphasis(Vec<MarkdownInline>),
    Code(String),
    Link {
        url: String,
        content: Vec<MarkdownInline>,
    },
    Image {
        url: String,
        alt: String,
    },
    Break,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Frontmatter {
    pub(crate) properties: Vec<FrontmatterProperty>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FrontmatterProperty {
    pub(crate) key: String,
    pub(crate) values: Vec<FrontmatterValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FrontmatterValue {
    Boolean(bool),
    Link { label: String, target: String },
    Text(String),
}

pub(crate) fn parse_markdown(text: &str) -> MarkdownDocument {
    let (frontmatter, body) = extract_frontmatter(text);
    let body = normalize_blockquote_depth(&body);
    let body = convert_wiki_links(&body);

    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);

    let mut events = Parser::new_ext(&body, options).peekable();
    MarkdownDocument {
        frontmatter: frontmatter.map(|content| parse_frontmatter(&content)),
        blocks: parse_blocks(&mut events, None),
    }
}

fn parse_blocks<'a, I>(events: &mut Peekable<I>, stop: Option<TagEnd>) -> Vec<MarkdownBlock>
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
                            items.push(parse_blocks(events, Some(TagEnd::Item)))
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

fn parse_frontmatter(content: &str) -> Frontmatter {
    let mut properties: Vec<FrontmatterProperty> = Vec::new();
    for line in content.lines() {
        if !line.starts_with(char::is_whitespace)
            && let Some((key, value)) = line.split_once(':')
        {
            properties.push(FrontmatterProperty {
                key: key.trim().to_string(),
                values: value
                    .trim()
                    .strip_prefix('[')
                    .and_then(|value| value.strip_suffix(']'))
                    .map(|value| {
                        value
                            .split(',')
                            .map(str::trim)
                            .filter(|v| !v.is_empty())
                            .collect()
                    })
                    .unwrap_or_else(|| {
                        let value = value.trim();
                        if value.is_empty() {
                            Vec::new()
                        } else {
                            vec![value]
                        }
                    })
                    .into_iter()
                    .map(parse_frontmatter_value)
                    .collect(),
            });
        } else if let Some(property) = properties.last_mut() {
            let value = line.trim().trim_start_matches('-').trim();
            if !value.is_empty() {
                property.values.push(parse_frontmatter_value(value));
            }
        }
    }
    Frontmatter { properties }
}

fn parse_frontmatter_value(value: &str) -> FrontmatterValue {
    match value {
        "true" => FrontmatterValue::Boolean(true),
        "false" => FrontmatterValue::Boolean(false),
        _ => parse_frontmatter_link(value).map_or_else(
            || FrontmatterValue::Text(value.to_string()),
            |(label, target)| FrontmatterValue::Link {
                label: label.to_string(),
                target: target.to_string(),
            },
        ),
    }
}

fn parse_frontmatter_link(value: &str) -> Option<(&str, &str)> {
    if let Some(link) = value.strip_prefix("[[").and_then(|v| v.strip_suffix("]]")) {
        return Some(link.split_once('|').unwrap_or((link, link)));
    }
    let markdown = value.strip_prefix('[')?.strip_suffix(')')?;
    markdown.split_once("](")
}

/// Make an explicit decrease in `>` markers close nested blockquotes.
///
/// CommonMark otherwise treats a line such as `> c` after `> > b` as a lazy
/// continuation of the inner quote. In a preview editor, following the visible
/// marker depth is less surprising.
fn normalize_blockquote_depth(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    let mut previous_depth = 0;

    for line in text.split_inclusive('\n') {
        let content = line.strip_suffix('\n').unwrap_or(line);
        let has_newline = line.ends_with('\n');
        let depth = blockquote_depth(content);

        if depth > 0 && depth < previous_depth {
            for level in 0..depth {
                if level > 0 {
                    normalized.push(' ');
                }
                normalized.push('>');
            }
            normalized.push('\n');
        }

        normalized.push_str(content);
        if has_newline {
            normalized.push('\n');
        }
        previous_depth = depth;
    }

    normalized
}

fn blockquote_depth(line: &str) -> usize {
    let mut rest = line.trim_start();
    let mut depth = 0;
    while let Some(after_marker) = rest.strip_prefix('>') {
        depth += 1;
        rest = after_marker.strip_prefix(' ').unwrap_or(after_marker);
    }
    depth
}

fn extract_frontmatter(text: &str) -> (Option<String>, String) {
    if !text.starts_with("---") {
        return (None, text.to_string());
    }

    let rest = &text[3..];
    if let Some(end) = rest.find("\n---") {
        let fm_content = &rest[..end];
        let body = if end + 4 < rest.len() {
            &rest[end + 4..]
        } else {
            ""
        };
        (Some(fm_content.trim().to_string()), body.to_string())
    } else {
        (None, text.to_string())
    }
}

fn convert_wiki_links(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '!' && chars.peek() == Some(&'[') {
            chars.next(); // consume first '['
            if chars.peek() == Some(&'[') {
                chars.next(); // consume second '['
                let mut link_text = String::new();
                let mut found_end = false;
                loop {
                    match chars.next() {
                        Some(']') => {
                            if chars.peek() == Some(&']') {
                                chars.next();
                                found_end = true;
                                break;
                            } else {
                                link_text.push(']');
                            }
                        }
                        Some(ch) => link_text.push(ch),
                        None => break,
                    }
                }
                if found_end {
                    if let Some(pipe_pos) = link_text.find('|') {
                        let display = &link_text[..pipe_pos];
                        let target = &link_text[pipe_pos + 1..];
                        let encoded = utf8_percent_encode(target, ENCODE_IN_LINK);
                        result.push_str(&format!("![{}]({})", display, encoded));
                    } else {
                        let encoded = utf8_percent_encode(&link_text, ENCODE_IN_LINK);
                        result.push_str(&format!("![{}]({})", link_text, encoded));
                    }
                } else {
                    result.push('!');
                    result.push_str("[[");
                    result.push_str(&link_text);
                }
            } else {
                // ![ but not ![[  — regular markdown image syntax, put back
                result.push('!');
                result.push('[');
            }
        } else if c == '[' && chars.peek() == Some(&'[') {
            chars.next();
            let mut link_text = String::new();
            let mut found_end = false;

            loop {
                match chars.next() {
                    Some(']') => {
                        if chars.peek() == Some(&']') {
                            chars.next();
                            found_end = true;
                            break;
                        } else {
                            link_text.push(']');
                        }
                    }
                    Some(ch) => link_text.push(ch),
                    None => break,
                }
            }

            if found_end {
                if let Some(pipe_pos) = link_text.find('|') {
                    let display = &link_text[..pipe_pos];
                    let target = &link_text[pipe_pos + 1..];
                    let encoded = utf8_percent_encode(target, ENCODE_IN_LINK);
                    result.push_str(&format!("[{}]({})", display, encoded));
                } else {
                    let encoded = utf8_percent_encode(&link_text, ENCODE_IN_LINK);
                    result.push_str(&format!("[{}]({})", link_text, encoded));
                }
            } else {
                result.push_str("[[");
                result.push_str(&link_text);
            }
        } else {
            result.push(c);
        }
    }

    result
}

pub(crate) fn find_link_at_offset(text: &str, offset: usize) -> Option<String> {
    let bytes = text.as_bytes();
    if offset >= bytes.len() {
        return None;
    }

    // Check markdown links [text](url)
    let mut pos = 0;
    while pos < bytes.len() {
        if let Some(link_start) = text[pos..].find('[') {
            let abs_start = pos + link_start;
            // Skip escaped brackets
            if abs_start > 0 && bytes[abs_start - 1] == b'\\' {
                pos = abs_start + 1;
                continue;
            }
            // Find closing bracket
            if let Some(bracket_end) = text[abs_start + 1..].find(']') {
                let bracket_end = abs_start + 1 + bracket_end;
                // Check if followed by (
                if bracket_end + 1 < bytes.len() && bytes[bracket_end + 1] == b'(' {
                    if let Some(paren_end) = find_matching_paren(text, bracket_end + 2) {
                        if offset >= abs_start && offset <= paren_end {
                            let url = &text[bracket_end + 2..paren_end];
                            return Some(url.to_string());
                        }
                        pos = paren_end + 1;
                        continue;
                    }
                }
            }
        }
        pos += 1;
    }

    // Check wiki links [[text]] or [[page|alias]]
    let mut pos = 0;
    while pos + 1 < bytes.len() {
        if bytes[pos] == b'[' && bytes[pos + 1] == b'[' {
            // Skip escaped
            if pos > 0 && bytes[pos - 1] == b'\\' {
                pos += 2;
                continue;
            }
            if let Some(end) = text[pos + 2..].find("]]") {
                let end = pos + 2 + end + 2;
                if offset >= pos && offset < end {
                    let inner = &text[pos + 2..end - 2];
                    let url = if let Some(pipe) = inner.find('|') {
                        &inner[pipe + 1..]
                    } else {
                        inner
                    };
                    return Some(url.to_string());
                }
                pos = end;
                continue;
            }
        }
        pos += 1;
    }

    None
}

fn find_matching_paren(text: &str, open_pos: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 1;
    let mut pos = open_pos;
    while pos < bytes.len() {
        match bytes[pos] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(pos);
                }
            }
            b'\\' if pos + 1 < bytes.len() => {
                pos += 1;
            }
            _ => {}
        }
        pos += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_blockquote_depth_decrease_closes_the_inner_quote() {
        assert_eq!(
            normalize_blockquote_depth("> a\n> > b\n> c"),
            "> a\n> > b\n>\n> c"
        );
    }

    #[test]
    fn parses_nested_document_structure() {
        let document = parse_markdown("# Title\n\n> 1. **bold** and [[Page|alias]]\n>    - child");

        assert!(
            matches!(
                document.blocks.as_slice(),
                [
                    MarkdownBlock::Heading { level: 1, .. },
                    MarkdownBlock::BlockQuote(blocks)
                ] if matches!(blocks.as_slice(), [MarkdownBlock::List { start: Some(1), items }]
                    if matches!(items[0].as_slice(), [MarkdownBlock::Paragraph(_), MarkdownBlock::List { start: None, .. }]))
            ),
            "{:#?}",
            document.blocks
        );
        let MarkdownBlock::BlockQuote(quoted) = &document.blocks[1] else {
            unreachable!()
        };
        let MarkdownBlock::List { items, .. } = &quoted[0] else {
            unreachable!()
        };
        assert_eq!(
            items[0][0],
            MarkdownBlock::Paragraph(vec![
                MarkdownInline::Strong(vec![MarkdownInline::Text("bold".into())]),
                MarkdownInline::Text(" and ".into()),
                MarkdownInline::Link {
                    url: "alias".into(),
                    content: vec![MarkdownInline::Text("Page".into())],
                },
            ])
        );
    }

    #[test]
    fn parses_typed_frontmatter_properties() {
        let document = parse_markdown(
            "---\npublished: true\nrelated:\n  - [[Page|Alias]]\ntags: [rust, markdown]\n---\nBody",
        );
        let frontmatter = document.frontmatter.expect("frontmatter");

        assert_eq!(
            frontmatter.properties,
            vec![
                FrontmatterProperty {
                    key: "published".into(),
                    values: vec![FrontmatterValue::Boolean(true)],
                },
                FrontmatterProperty {
                    key: "related".into(),
                    values: vec![FrontmatterValue::Link {
                        label: "Page".into(),
                        target: "Alias".into(),
                    }],
                },
                FrontmatterProperty {
                    key: "tags".into(),
                    values: vec![
                        FrontmatterValue::Text("rust".into()),
                        FrontmatterValue::Text("markdown".into()),
                    ],
                },
            ]
        );
    }
}
