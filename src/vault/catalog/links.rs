use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct WikiLinkOccurrence {
    pub(super) target: String,
    pub(super) target_range: Range<usize>,
    pub(super) explicit_md_extension: bool,
}

pub(super) fn normalize_target(name: &str) -> String {
    name.split('|')
        .next()
        .unwrap_or(name)
        .split('#')
        .next()
        .unwrap_or(name)
        .trim()
        .trim_start_matches('/')
        .replace('\\', "/")
}

pub(super) fn extract_wiki_link_occurrences(source: &str) -> Vec<WikiLinkOccurrence> {
    let mut links = Vec::new();
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
        let mut index = 0;
        let mut inline_code = false;
        while index < bytes.len() {
            if bytes[index] == b'`' {
                inline_code = !inline_code;
                index += 1;
                continue;
            }
            if !inline_code && bytes[index..].starts_with(b"[[") {
                if let Some(end) = line[index + 2..].find("]]") {
                    let content_start = index + 2;
                    let content_end = content_start + end;
                    let authored = &line[content_start..content_end];
                    let target_end = authored.find(['#', '|']).unwrap_or(authored.len());
                    let target_authored = &authored[..target_end];
                    let target = normalize_target(target_authored);
                    if !target.is_empty() {
                        let leading = target_authored.len() - target_authored.trim_start().len();
                        let trailing = target_authored.trim_end().len();
                        links.push(WikiLinkOccurrence {
                            explicit_md_extension: target.ends_with(".md"),
                            target,
                            target_range: (line_start + content_start + leading)
                                ..(line_start + content_start + trailing),
                        });
                    }
                    index = content_end + 2;
                    continue;
                }
            }
            index += 1;
        }
        line_start += line_with_ending.len();
    }
    links
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_links_and_embeds_but_not_code() {
        let source = "---\nrelated: [[Frontmatter]]\n---\n[[Body]] ![[Embed]] `[[Inline]]`\n```md\n[[Fence]]\n```";
        let links = extract_wiki_link_occurrences(source);
        assert_eq!(
            links
                .iter()
                .map(|link| link.target.as_str())
                .collect::<Vec<_>>(),
            vec!["Frontmatter", "Body", "Embed"]
        );
        for link in links {
            assert_eq!(&source[link.target_range], link.target);
        }
    }

    #[test]
    fn ranges_cover_only_target_and_preserve_suffix() {
        let source = "[[ Note.md#Heading|Alias ]]";
        let link = extract_wiki_link_occurrences(source).pop().unwrap();
        assert_eq!(link.target, "Note.md");
        assert_eq!(&source[link.target_range], "Note.md");
        assert!(link.explicit_md_extension);
    }
}
