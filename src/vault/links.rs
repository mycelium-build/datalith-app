use std::ops::Range;

#[derive(Clone, Debug)]
pub struct LinkOccurrence {
    pub(crate) target: String,
    pub(crate) range: Range<usize>,
}

pub fn normalized_target(authored: &str) -> String {
    authored
        .split(['|', '#'])
        .next()
        .unwrap_or(authored)
        .trim()
        .trim_start_matches('/')
        .replace('\\', "/")
}

// Byte-cursor arithmetic below is bounded by the length of each line and cannot overflow.
#[allow(clippy::arithmetic_side_effects)]
pub fn occurrences(source: &str) -> Vec<LinkOccurrence> {
    let mut result = Vec::new();
    let mut fenced = false;
    let mut line_start = 0;
    for line_with_ending in source.split_inclusive('\n') {
        let line = line_with_ending
            .strip_suffix('\n')
            .unwrap_or(line_with_ending);
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            line_start += line_with_ending.len();
            continue;
        }
        if fenced {
            line_start += line_with_ending.len();
            continue;
        }
        let bytes = line.as_bytes();
        let mut cursor = 0;
        let mut inline = false;
        while cursor < bytes.len() {
            if bytes.get(cursor) == Some(&b'`') {
                inline = !inline;
                cursor += 1;
                continue;
            }
            if !inline
                && bytes
                    .get(cursor..)
                    .is_some_and(|suffix| suffix.starts_with(b"[["))
                && let Some(length) = line.get(cursor + 2..).and_then(|suffix| suffix.find("]]"))
            {
                let content_start = cursor + 2;
                let content_end = content_start + length;
                let authored = line.get(content_start..content_end).unwrap_or("");
                let target_length = authored.find(['|', '#']).unwrap_or(authored.len());
                let raw = authored.get(..target_length).unwrap_or(authored);
                let target = normalized_target(raw);
                if !target.is_empty() {
                    let leading = raw.len() - raw.trim_start().len();
                    let trailing = raw.trim_end().len();
                    result.push(LinkOccurrence {
                        target,
                        range: line_start + content_start + leading
                            ..line_start + content_start + trailing,
                    });
                }
                cursor = content_end + 2;
                continue;
            }
            cursor += 1;
        }
        line_start += line_with_ending.len();
    }
    result
}

pub fn rewrite(source: &str, replacements: &[(usize, String)]) -> String {
    let occurrences = occurrences(source);
    let mut edits = replacements
        .iter()
        .filter_map(|(ordinal, replacement)| {
            occurrences
                .get(*ordinal)
                .map(|link| (link.range.clone(), replacement))
        })
        .collect::<Vec<_>>();
    edits.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    let mut result = source.to_owned();
    for (range, replacement) in edits {
        result.replace_range(range, replacement);
    }
    result
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::indexing_slicing,
        clippy::string_slice
    )]
    use super::*;

    #[test]
    fn extracts_links_and_embeds_outside_code() {
        let source = "[[A]] ![[folder/B|image]] `[[inline]]`\n```md\n[[fenced]]\n```";
        let found = occurrences(source);
        assert_eq!(
            found
                .iter()
                .map(|link| link.target.as_str())
                .collect::<Vec<_>>(),
            ["A", "folder/B"]
        );
        assert_eq!(&source[found[1].range.clone()], "folder/B");
    }
}
