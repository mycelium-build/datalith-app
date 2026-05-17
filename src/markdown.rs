use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

#[derive(Clone, Debug)]
pub enum MarkdownEvent {
    Text(String, MarkdownStyle),
    BlockStart(MarkdownBlock),
    BlockEnd,
    LinkStart(String),
    LinkEnd,
}

#[derive(Clone, Debug)]
pub enum MarkdownBlock {
    Heading,
    Paragraph,
    List(bool, usize),
    ListItem(usize),
    BlockQuote,
    Code(String),
    Frontmatter(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum MarkdownStyle {
    Heading(u32),
    Bold,
    Italic,
    BoldItalic,
    Code,
    Link,
    Normal,
}

pub fn parse_markdown(text: &str) -> Vec<MarkdownEvent> {
    let (frontmatter, body) = extract_frontmatter(text);
    let body = convert_wiki_links(&body);

    let mut events = Vec::new();

    if let Some(fm) = frontmatter {
        events.push(MarkdownEvent::BlockStart(MarkdownBlock::Frontmatter(fm)));
        events.push(MarkdownEvent::BlockEnd);
    }

    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);

    let parser = Parser::new_ext(&body, options);
    let mut style_stack: Vec<MarkdownStyle> = vec![MarkdownStyle::Normal];
    let mut text_buffer = String::new();
    let mut in_code_block = false;
    let mut code_block_content = String::new();
    let mut list_depth: usize = 0;

    for (event, _range) in parser.into_offset_iter() {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    flush_text(&mut text_buffer, &style_stack, &mut events);
                    let lvl = match level {
                        pulldown_cmark::HeadingLevel::H1 => 1,
                        pulldown_cmark::HeadingLevel::H2 => 2,
                        pulldown_cmark::HeadingLevel::H3 => 3,
                        pulldown_cmark::HeadingLevel::H4 => 4,
                        pulldown_cmark::HeadingLevel::H5 => 5,
                        pulldown_cmark::HeadingLevel::H6 => 6,
                    };
                    events.push(MarkdownEvent::BlockStart(MarkdownBlock::Heading));
                    style_stack.push(MarkdownStyle::Heading(lvl));
                }
                Tag::Paragraph => {
                    flush_text(&mut text_buffer, &style_stack, &mut events);
                    events.push(MarkdownEvent::BlockStart(MarkdownBlock::Paragraph));
                }
                Tag::Strong => {
                    style_stack.push(MarkdownStyle::Bold);
                }
                Tag::Emphasis => {
                    style_stack.push(MarkdownStyle::Italic);
                }
                Tag::Link { dest_url, .. } => {
                    flush_text(&mut text_buffer, &style_stack, &mut events);
                    events.push(MarkdownEvent::LinkStart(dest_url.to_string()));
                    style_stack.push(MarkdownStyle::Link);
                }
                Tag::CodeBlock(_) => {
                    flush_text(&mut text_buffer, &style_stack, &mut events);
                    in_code_block = true;
                    code_block_content.clear();
                }
                Tag::List(checked) => {
                    flush_text(&mut text_buffer, &style_stack, &mut events);
                    list_depth += 1;
                    events.push(MarkdownEvent::BlockStart(MarkdownBlock::List(
                        checked.is_some(),
                        list_depth,
                    )));
                }
                Tag::Item => {
                    flush_text(&mut text_buffer, &style_stack, &mut events);
                    events.push(MarkdownEvent::BlockStart(MarkdownBlock::ListItem(
                        list_depth,
                    )));
                }
                Tag::BlockQuote(_) => {
                    flush_text(&mut text_buffer, &style_stack, &mut events);
                    events.push(MarkdownEvent::BlockStart(MarkdownBlock::BlockQuote));
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Heading(_) => {
                    flush_text(&mut text_buffer, &style_stack, &mut events);
                    events.push(MarkdownEvent::BlockEnd);
                    style_stack.retain(|s| !matches!(s, MarkdownStyle::Heading(_)));
                }
                TagEnd::Paragraph => {
                    flush_text(&mut text_buffer, &style_stack, &mut events);
                    events.push(MarkdownEvent::BlockEnd);
                }
                TagEnd::Strong | TagEnd::Emphasis => {
                    flush_text(&mut text_buffer, &style_stack, &mut events);
                    if style_stack.len() > 1 {
                        style_stack.pop();
                    }
                }
                TagEnd::Link => {
                    flush_text(&mut text_buffer, &style_stack, &mut events);
                    events.push(MarkdownEvent::LinkEnd);
                    if style_stack.len() > 1 {
                        style_stack.pop();
                    }
                }
                TagEnd::CodeBlock => {
                    events.push(MarkdownEvent::BlockStart(MarkdownBlock::Code(
                        code_block_content.clone(),
                    )));
                    events.push(MarkdownEvent::BlockEnd);
                    in_code_block = false;
                    code_block_content.clear();
                }
                TagEnd::List(_) => {
                    events.push(MarkdownEvent::BlockEnd);
                    list_depth = list_depth.saturating_sub(1);
                }
                TagEnd::Item => {
                    flush_text(&mut text_buffer, &style_stack, &mut events);
                    events.push(MarkdownEvent::BlockEnd);
                }
                TagEnd::BlockQuote(_) => {
                    flush_text(&mut text_buffer, &style_stack, &mut events);
                    events.push(MarkdownEvent::BlockEnd);
                }
                _ => {}
            },
            Event::Text(t) => {
                if in_code_block {
                    code_block_content.push_str(&t);
                } else {
                    text_buffer.push_str(&t);
                }
            }
            Event::Code(t) => {
                events.push(MarkdownEvent::Text(t.to_string(), MarkdownStyle::Code));
            }
            Event::SoftBreak | Event::HardBreak => {
                if in_code_block {
                    code_block_content.push('\n');
                } else {
                    text_buffer.push(' ');
                }
            }
            Event::Rule => {
                flush_text(&mut text_buffer, &style_stack, &mut events);
            }
            Event::Html(t) | Event::InlineHtml(t) => {
                text_buffer.push_str(&t);
            }
            _ => {}
        }
    }

    flush_text(&mut text_buffer, &style_stack, &mut events);

    events
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
        if c == '[' && chars.peek() == Some(&'[') {
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
                    result.push_str(&format!("[{}]({})", display, target));
                } else {
                    result.push_str(&format!("[{}]({})", link_text, link_text));
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

fn flush_text(buffer: &mut String, style_stack: &[MarkdownStyle], events: &mut Vec<MarkdownEvent>) {
    if !buffer.is_empty() {
        let style = composite_style(style_stack);
        events.push(MarkdownEvent::Text(buffer.clone(), style));
        buffer.clear();
    }
}

fn composite_style(stack: &[MarkdownStyle]) -> MarkdownStyle {
    let mut has_bold = false;
    let mut has_italic = false;
    let mut has_link = false;
    let mut heading: Option<u32> = None;

    for style in stack {
        match style {
            MarkdownStyle::Bold => has_bold = true,
            MarkdownStyle::Italic => has_italic = true,
            MarkdownStyle::Link => has_link = true,
            MarkdownStyle::Heading(level) => heading = Some(*level),
            _ => {}
        }
    }

    if let Some(level) = heading {
        return MarkdownStyle::Heading(level);
    }

    if has_link {
        return MarkdownStyle::Link;
    }

    if has_bold && has_italic {
        return MarkdownStyle::BoldItalic;
    }
    if has_bold {
        return MarkdownStyle::Bold;
    }
    if has_italic {
        return MarkdownStyle::Italic;
    }

    MarkdownStyle::Normal
}
