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
        let (content, newline) = match line.strip_suffix('\n') {
            Some(content) => (content, "\n"),
            None => (line, ""),
        };

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
                            let target = &link_text[..pipe_pos];
                            let display = &link_text[pipe_pos + 1..];
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
                        let target = &link_text[..pipe_pos];
                        let display = &link_text[pipe_pos + 1..];
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
        result.push_str(newline);
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
    fn converts_wiki_link_alias_to_display_and_target() {
        let converted = convert_wiki_links("See [[not this|but this]] and [[Page]]");
        assert_eq!(converted, "See [but this](not%20this) and [Page](Page)");
    }

    #[test]
    fn leaves_wiki_links_inside_code_blocks_untouched() {
        let converted = convert_wiki_links(
            "Text [[Page]]\n```\n[[code]]\n```\n`[[inline]]` and [[Page]]",
        );
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
