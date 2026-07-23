use std::ops::Range;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub(crate) struct LinkOccurrence {
    pub(crate) target: String,
    pub(crate) range: Range<usize>,
}

pub(crate) fn normalized_target(authored: &str) -> String {
    authored
        .split(['|', '#'])
        .next()
        .unwrap_or(authored)
        .trim()
        .trim_start_matches('/')
        .replace('\\', "/")
}

pub(crate) fn occurrences(source: &str) -> Vec<LinkOccurrence> {
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
            if bytes[cursor] == b'`' {
                inline = !inline;
                cursor += 1;
                continue;
            }
            if !inline
                && bytes[cursor..].starts_with(b"[[")
                && let Some(length) = line[cursor + 2..].find("]]")
            {
                let content_start = cursor + 2;
                let content_end = content_start + length;
                let authored = &line[content_start..content_end];
                let target_length = authored.find(['|', '#']).unwrap_or(authored.len());
                let raw = &authored[..target_length];
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

pub(crate) fn rewrite(source: &str, replacements: &[(usize, String)]) -> String {
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

pub(crate) fn resolve<'a>(
    authored: &str,
    root: &Path,
    paths: impl IntoIterator<Item = &'a PathBuf>,
) -> Option<PathBuf> {
    let target = normalized_target(authored);
    if target.is_empty() {
        return None;
    }
    let target_path = Path::new(&target);
    let qualified = target.contains('/');
    let target_has_extension = target_path.extension().is_some();
    let mut candidates = paths
        .into_iter()
        .filter_map(|path| {
            let relative = path.strip_prefix(root).ok()?;
            let matches = if qualified {
                if target_has_extension {
                    relative.to_string_lossy().eq_ignore_ascii_case(&target)
                } else {
                    relative
                        .with_extension("")
                        .to_string_lossy()
                        .eq_ignore_ascii_case(&target)
                }
            } else if target_has_extension {
                relative
                    .file_name()?
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&target)
            } else {
                relative
                    .file_stem()?
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&target)
            };
            matches.then_some(path.clone())
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        let left_relative = left.strip_prefix(root).unwrap_or(left);
        let right_relative = right.strip_prefix(root).unwrap_or(right);
        left_relative
            .components()
            .count()
            .cmp(&right_relative.components().count())
            .then_with(|| is_markdown(right).cmp(&is_markdown(left)))
            .then_with(|| {
                left_relative
                    .to_string_lossy()
                    .to_lowercase()
                    .cmp(&right_relative.to_string_lossy().to_lowercase())
            })
            .then_with(|| left_relative.cmp(right_relative))
    });
    candidates.into_iter().next()
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn ambiguous_links_prefer_shallow_then_markdown_then_alphabetic() {
        let root = PathBuf::from("/vault");
        let paths = vec![root.join("Note.txt"), root.join("a/Note.md")];
        assert_eq!(
            resolve("Note", &root, paths.iter()),
            Some(root.join("Note.txt"))
        );

        let same_depth = vec![
            root.join("z/Note.txt"),
            root.join("b/Note.md"),
            root.join("a/Note.md"),
        ];
        assert_eq!(
            resolve("Note", &root, same_depth.iter()),
            Some(root.join("a/Note.md"))
        );
    }
}
