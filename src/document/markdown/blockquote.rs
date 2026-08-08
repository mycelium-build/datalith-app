/// Make an explicit decrease in `>` markers close nested blockquotes.
///
/// `CommonMark` otherwise treats a line such as `> c` after `> > b` as a lazy
/// continuation of the inner quote. In a preview editor, following the visible
/// marker depth is less surprising.
pub(super) fn normalize_blockquote_depth(text: &str) -> String {
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
    let mut depth: usize = 0;
    while let Some(after_marker) = rest.strip_prefix('>') {
        let Some(next_depth) = depth.checked_add(1) else {
            break;
        };
        depth = next_depth;
        rest = after_marker.strip_prefix(' ').unwrap_or(after_marker);
    }
    depth
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
}
