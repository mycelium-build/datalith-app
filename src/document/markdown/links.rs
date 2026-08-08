use std::fmt::Write as _;
use std::iter::Peekable;
use std::str::Chars;

use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};

const ENCODE_IN_LINK: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'<').add(b'>').add(b'`');

fn fence_at(line: &str) -> Option<(char, usize)> {
    let trimmed = line.trim_start();
    let c = trimmed.chars().next()?;
    if c != '`' && c != '~' {
        return None;
    }
    let len = trimmed.chars().take_while(|&ch| ch == c).count();
    (len >= 3).then_some((c, len))
}

pub(super) fn convert_wiki_links(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut fence: Option<(char, usize)> = None;

    for line in text.split_inclusive('\n') {
        let (content, newline) = line
            .strip_suffix('\n')
            .map_or((line, ""), |content| (content, "\n"));

        if let Some((fence_char, fence_len)) = fence {
            if let Some((c, len)) = fence_at(content)
                && c == fence_char
                && len >= fence_len
            {
                fence = None;
            }
            result.push_str(line);
            continue;
        } else if let Some((c, len)) = fence_at(content) {
            fence = Some((c, len));
            result.push_str(line);
            continue;
        }

        convert_inline_links(content, &mut result);
        result.push_str(newline);
    }

    result
}

fn convert_inline_links(content: &str, result: &mut String) {
    let mut inline = false;
    let mut chars = content.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '`' {
            inline = !inline;
            result.push(c);
            continue;
        }
        if inline {
            result.push(c);
            continue;
        }
        if c == '!' && chars.peek() == Some(&'[') {
            chars.next();
            if chars.peek() == Some(&'[') {
                chars.next();
                consume_wiki_link(&mut chars, true, result);
            } else {
                result.push('!');
                result.push('[');
            }
        } else if c == '[' && chars.peek() == Some(&'[') {
            chars.next();
            consume_wiki_link(&mut chars, false, result);
        } else {
            result.push(c);
        }
    }
}

fn consume_wiki_link(chars: &mut Peekable<Chars<'_>>, is_image: bool, result: &mut String) {
    let mut link_text = String::new();
    let mut found_end = false;
    loop {
        match chars.next() {
            Some(']') => {
                if chars.peek() == Some(&']') {
                    chars.next();
                    found_end = true;
                    break;
                }
                link_text.push(']');
            }
            Some(ch) => link_text.push(ch),
            None => break,
        }
    }
    if !found_end {
        if is_image {
            result.push('!');
        }
        result.push_str("[[");
        result.push_str(&link_text);
        return;
    }
    let (target, display) = link_text
        .split_once('|')
        .unwrap_or((link_text.as_str(), link_text.as_str()));
    let encoded = utf8_percent_encode(target, ENCODE_IN_LINK);
    if is_image {
        let _ = write!(result, "![{display}]({encoded})");
    } else {
        let _ = write!(result, "[{display}]({encoded})");
    }
}

/// Finds the URL of the link covering `offset` in `text`, if any.
///
/// Scans for both Markdown `[text](url)` links and wiki `[[target]]` links.
pub fn find_link_at_offset(text: &str, offset: usize) -> Option<String> {
    let bytes = text.as_bytes();
    if offset >= bytes.len() {
        return None;
    }

    // Check markdown links [text](url).
    let mut pos = 0;
    // Byte offsets are bounded by the string length, so the arithmetic below cannot overflow.
    #[allow(clippy::arithmetic_side_effects)]
    while pos < bytes.len() {
        let Some(relative) = bytes
            .get(pos..)
            .and_then(|rest| rest.iter().position(|&b| b == b'['))
        else {
            break;
        };
        let link_start = pos + relative;
        // Skip escaped brackets
        if link_start > 0 && bytes.get(link_start - 1) == Some(&b'\\') {
            pos = link_start + 1;
            continue;
        }
        // Find closing bracket followed by (
        if let Some(bracket_end) = bytes
            .get(link_start + 1..)
            .and_then(|rest| rest.iter().position(|&b| b == b']'))
            .map(|relative| link_start + 1 + relative)
            && bytes.get(bracket_end + 1) == Some(&b'(')
            && let Some(paren_end) = find_matching_paren(text, bracket_end + 2)
        {
            if offset >= link_start && offset <= paren_end {
                let url = text.get(bracket_end + 2..paren_end)?;
                return Some(url.to_string());
            }
            pos = paren_end + 1;
            continue;
        }
        pos += 1;
    }

    // Check wiki links [[text]] or [[page|alias]]
    let mut pos = 0;
    #[allow(clippy::arithmetic_side_effects)]
    while pos + 1 < bytes.len() {
        if bytes.get(pos) == Some(&b'[') && bytes.get(pos + 1) == Some(&b'[') {
            // Skip escaped
            if pos > 0 && bytes.get(pos - 1) == Some(&b'\\') {
                pos += 2;
                continue;
            }
            if let Some(end) = text
                .get(pos + 2..)
                .and_then(|rest| rest.find("]]"))
                .map(|end| pos + 2 + end + 2)
            {
                if offset >= pos
                    && offset < end
                    && let Some(inner) = text.get(pos + 2..end - 2)
                {
                    let url = inner.split(['|', '#']).next().unwrap_or(inner);
                    return Some(url.trim().to_string());
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
    // Byte offsets are bounded by the string length, so the arithmetic below cannot overflow.
    #[allow(clippy::arithmetic_side_effects)]
    while pos < bytes.len() {
        match bytes.get(pos) {
            Some(b'(') => depth += 1,
            Some(b')') => {
                depth -= 1;
                if depth == 0 {
                    return Some(pos);
                }
            }
            Some(b'\\') if bytes.get(pos + 1).is_some() => {
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
    fn converts_wiki_link_alias_to_display_and_target() {
        let converted = convert_wiki_links("See [[not this|but this]] and [[Page]]");
        assert_eq!(converted, "See [but this](not%20this) and [Page](Page)");
    }

    #[test]
    fn leaves_wiki_links_inside_code_blocks_untouched() {
        let converted =
            convert_wiki_links("Text [[Page]]\n```\n[[code]]\n```\n`[[inline]]` and [[Page]]");
        assert_eq!(
            converted,
            "Text [Page](Page)\n```\n[[code]]\n```\n`[[inline]]` and [Page](Page)"
        );
    }

    #[test]
    fn finds_target_before_pipe_in_wiki_link() {
        let text = "[[not this|but this]]";
        let url = find_link_at_offset(text, 5).unwrap();
        assert_eq!(url, "not this");
    }
}
