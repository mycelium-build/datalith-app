use std::fmt::Write as _;
use std::iter::Peekable;
use std::str::CharIndices;

use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};

const ENCODE_IN_LINK: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'<').add(b'>').add(b'`');

fn wiki_link_target(inner: &str) -> &str {
    inner.split(['|', '#']).next().unwrap_or(inner).trim()
}

/// A fenced code block boundary on `line`, when the line is a valid opening/closing fence.
///
/// `indent` must be at most 3 spaces and the line must start with a run of at least three backticks or tildes.
/// `rest` is the line content after the run.
fn fence_at(line: &str) -> Option<(char, usize, &str)> {
    let indent = leading_indent_columns(line);
    if indent > 3 {
        return None;
    }
    let trimmed = line.trim_start_matches([' ', '\t']);
    let c = trimmed.chars().next()?;
    if c != '`' && c != '~' {
        return None;
    }
    let len = trimmed.chars().take_while(|&ch| ch == c).count();
    if len < 3 {
        return None;
    }
    let rest = trimmed.get(len..)?;
    Some((c, len, rest))
}

/// Byte-indexed mask marking every position that lies inside a fenced code block,
/// an indented code block, or an inline code span,
/// mirroring the Markdown parser so wiki links there stay literal.
///
/// An indented code block cannot interrupt a paragraph,
/// so it only starts after a blank line (or at the start of the text)
/// and continues across blank lines until a non-blank line indented fewer than four columns appears.
fn code_mask(text: &str) -> Vec<bool> {
    let mut mask = vec![false; text.len()];
    let mut fence: Option<(char, usize)> = None;
    let mut indented_code = false;
    let mut previous_blank = true;
    let mut line_start = 0usize;
    for line in text.split_inclusive('\n') {
        let content = line.strip_suffix('\n').unwrap_or(line);
        let blank = content.trim().is_empty();
        if let Some((fence_char, fence_len)) = fence {
            let closes = fence_at(content).is_some_and(|(c, len, rest)| {
                c == fence_char && len >= fence_len && rest.chars().all(char::is_whitespace)
            });
            if closes {
                fence = None;
            } else {
                mark_line(content, line_start, &mut mask);
            }
        } else if indented_code {
            if blank {
                // A blank line inside an indented code block keeps the block open.
            } else if leading_indent_columns(content) >= 4 {
                mark_line(content, line_start, &mut mask);
            } else {
                indented_code = false;
                scan_code_line(content, line_start, &mut mask, &mut fence);
            }
        } else if !blank && leading_indent_columns(content) >= 4 && previous_blank {
            indented_code = true;
            mark_line(content, line_start, &mut mask);
        } else {
            scan_code_line(content, line_start, &mut mask, &mut fence);
        }
        previous_blank = blank;
        line_start = line_start.saturating_add(line.len());
    }
    mask
}

fn mark_line(content: &str, line_start: usize, mask: &mut [bool]) {
    if let Some(slice) = mask.get_mut(line_start..line_start.saturating_add(content.len())) {
        slice.fill(true);
    }
}

fn scan_code_line(
    content: &str,
    line_start: usize,
    mask: &mut [bool],
    fence: &mut Option<(char, usize)>,
) {
    if let Some((c, len, _)) = fence_at(content) {
        *fence = Some((c, len));
    } else {
        scan_inline_code(content, line_start, mask);
    }
}

/// Counts the leading indentation of `line` in columns, treating a tab as four spaces.
fn leading_indent_columns(line: &str) -> usize {
    let mut columns = 0usize;
    for c in line.chars() {
        match c {
            ' ' => columns = columns.saturating_add(1),
            '\t' => columns = columns.saturating_add(4),
            _ => break,
        }
    }
    columns
}

/// Marks inline code spans delimited by backtick runs
/// (a run of *n* backticks closes a span opened by a run of *n* backticks),
/// so links inside `code` stay literal.
fn scan_inline_code(content: &str, line_start: usize, mask: &mut [bool]) {
    let bytes = content.as_bytes();
    let mut index = 0usize;
    let mut in_code = false;
    let mut delimiter_len = 0usize;
    while index < bytes.len() {
        if bytes.get(index) == Some(&b'`') {
            let run = bytes.get(index..).map_or(0, |rest| {
                rest.iter().take_while(|&&byte| byte == b'`').count()
            });
            if in_code {
                if run == delimiter_len {
                    in_code = false;
                }
            } else {
                in_code = true;
                delimiter_len = run;
            }
            for offset in index..index.saturating_add(run) {
                if let Some(slot) = mask.get_mut(line_start.saturating_add(offset)) {
                    *slot = true;
                }
            }
            index = index.saturating_add(run);
        } else {
            if in_code && let Some(slot) = mask.get_mut(line_start.saturating_add(index)) {
                *slot = true;
            }
            index = index.saturating_add(1);
        }
    }
}

pub(super) fn convert_wiki_links(text: &str) -> String {
    let mask = code_mask(text);
    let mut result = String::with_capacity(text.len());
    let mut chars = text.char_indices().peekable();
    while let Some((index, c)) = chars.next() {
        if mask.get(index).copied().unwrap_or(false) {
            result.push(c);
            continue;
        }
        if c == '!' && chars.peek().is_some_and(|&(_, next)| next == '[') {
            chars.next();
            if chars.peek().is_some_and(|&(_, next)| next == '[') {
                chars.next();
                consume_wiki_link(&mut chars, true, &mut result);
            } else {
                result.push('!');
                result.push('[');
            }
        } else if c == '[' && chars.peek().is_some_and(|&(_, next)| next == '[') {
            chars.next();
            consume_wiki_link(&mut chars, false, &mut result);
        } else {
            result.push(c);
        }
    }
    result
}

fn consume_wiki_link(chars: &mut Peekable<CharIndices<'_>>, is_image: bool, result: &mut String) {
    let mut link_text = String::new();
    let mut found_end = false;
    while let Some((_, c)) = chars.next() {
        if c == ']' {
            if chars.peek().is_some_and(|&(_, next)| next == ']') {
                chars.next();
                found_end = true;
                break;
            }
            link_text.push(']');
        } else {
            link_text.push(c);
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
    let encoded = utf8_percent_encode(wiki_link_target(target), ENCODE_IN_LINK);
    if is_image {
        let _ = write!(result, "![{display}]({encoded})");
    } else {
        let _ = write!(result, "[{display}]({encoded})");
    }
}

/// Finds the URL of the link covering `offset` in `text`, if any.
///
/// Scans for both Markdown `[text](url)` links and wiki `[[target]]` links,
/// skipping anything inside code fences or inline code spans.
pub fn find_link_at_offset(text: &str, offset: usize) -> Option<String> {
    let bytes = text.as_bytes();
    if offset >= bytes.len() {
        return None;
    }
    let mask = code_mask(text);

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
        // Skip links inside code
        if mask.get(link_start).copied().unwrap_or(false) {
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
            // Skip links inside code
            if mask.get(pos).copied().unwrap_or(false) {
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
                    return Some(wiki_link_target(inner).to_string());
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
    fn trims_the_target_and_drops_anchor_from_wiki_links() {
        assert_eq!(
            convert_wiki_links("[[ Page # Section |Alias]]"),
            "[Alias](Page)"
        );
        assert_eq!(
            find_link_at_offset("[[ Page # Section ]]", 5),
            Some("Page".to_string())
        );
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
    fn code_fence_only_closes_on_a_whitespace_only_fence_line() {
        let converted = convert_wiki_links("```\n[[code]]\n``` trailing\n[[outside]]");
        assert_eq!(converted, "```\n[[code]]\n``` trailing\n[[outside]]");
    }

    #[test]
    fn finds_target_before_pipe_in_wiki_link() {
        let text = "[[not this|but this]]";
        let url = find_link_at_offset(text, 5).unwrap();
        assert_eq!(url, "not this");
    }

    #[test]
    fn find_link_ignores_wiki_links_inside_code_fences() {
        let text = "```\n[[hidden]]\n```\n[[visible]]";
        assert_eq!(find_link_at_offset(text, 8), None);
        let visible_start = text.find("[[visible]]").unwrap();
        assert_eq!(
            find_link_at_offset(text, visible_start + 2),
            Some("visible".to_string())
        );
    }

    #[test]
    fn find_link_ignores_links_inside_inline_code() {
        let text = "`[[hidden]]` [[visible]]";
        assert_eq!(find_link_at_offset(text, 4), None);
        let visible_start = text.find("[[visible]]").unwrap();
        assert_eq!(
            find_link_at_offset(text, visible_start + 2),
            Some("visible".to_string())
        );
        let markdown = "`[link](url)` [link](url)";
        assert_eq!(find_link_at_offset(markdown, 3), None);
        assert_eq!(find_link_at_offset(markdown, 17), Some("url".to_string()));
    }

    #[test]
    fn inline_code_span_with_inner_backticks_stays_literal() {
        let converted = convert_wiki_links("`` `[[code]]` `` and [[Page]]");
        assert_eq!(converted, "`` `[[code]]` `` and [Page](Page)");
    }

    #[test]
    fn leaves_wiki_links_inside_indented_code_blocks_untouched() {
        assert_eq!(convert_wiki_links("    [[a]]"), "    [[a]]");
        assert_eq!(
            convert_wiki_links("text\n\n    [[a]]\n    more"),
            "text\n\n    [[a]]\n    more"
        );
    }

    #[test]
    fn indented_code_continues_across_blank_lines_until_indentation_drops() {
        let converted = convert_wiki_links("    [[a]]\n\n    [[b]]\nnot code\n    [[c]]");
        assert_eq!(converted, "    [[a]]\n\n    [[b]]\nnot code\n    [c](c)");
        assert_eq!(
            convert_wiki_links("text\n\n    [[a]]\n\n    [[b]]"),
            "text\n\n    [[a]]\n\n    [[b]]"
        );
    }

    #[test]
    fn indented_code_cannot_interrupt_a_paragraph() {
        assert_eq!(
            convert_wiki_links("paragraph\n    [[a]]"),
            "paragraph\n    [a](a)"
        );
        assert_eq!(
            find_link_at_offset("paragraph\n    [[a]]", 18),
            Some("a".to_string())
        );
        assert_eq!(find_link_at_offset("    [[a]]", 6), None);
    }
}
