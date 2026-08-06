mod blockquote;
mod frontmatter;
mod links;
mod parse;

pub use self::frontmatter::{Frontmatter, FrontmatterValue};
pub use links::find_link_at_offset;

#[derive(Clone, Debug, PartialEq)]
pub struct MarkdownDocument {
    pub(crate) frontmatter: Option<Frontmatter>,
    pub(crate) blocks: Vec<MarkdownBlock>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MarkdownBlock {
    Heading {
        level: u32,
        content: Vec<MarkdownInline>,
    },
    Paragraph(Vec<MarkdownInline>),
    List {
        start: Option<u64>,
        items: Vec<ListItem>,
    },
    BlockQuote(Vec<Self>),
    Table {
        headers: Vec<Vec<MarkdownInline>>,
        rows: Vec<Vec<Vec<MarkdownInline>>>,
    },
    Code {
        language: Option<String>,
        content: String,
    },
    Rule,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ListItem {
    pub(crate) task: Option<bool>,
    pub(crate) blocks: Vec<MarkdownBlock>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MarkdownInline {
    Text(String),
    Strong(Vec<Self>),
    Emphasis(Vec<Self>),
    Code(String),
    Link { url: String, content: Vec<Self> },
    Image { url: String, alt: String },
    Break,
}

pub fn parse_markdown(text: &str) -> MarkdownDocument {
    let (frontmatter, body) = frontmatter::extract_frontmatter(text);
    let body = blockquote::normalize_blockquote_depth(&body);
    let body = links::convert_wiki_links(&body);

    let mut options = pulldown_cmark::Options::empty();
    options.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    options.insert(pulldown_cmark::Options::ENABLE_TABLES);
    options.insert(pulldown_cmark::Options::ENABLE_TASKLISTS);

    let mut events = pulldown_cmark::Parser::new_ext(&body, options).peekable();
    MarkdownDocument {
        frontmatter: frontmatter.map(|content| frontmatter::parse_frontmatter(&content)),
        blocks: parse::parse_blocks(&mut events, None),
    }
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
    fn parses_nested_document_structure() {
        let document = parse_markdown("# Title\n\n> 1. **bold** and [[Page|alias]]\n>    - child");

        assert!(
            matches!(
                document.blocks.as_slice(),
                [
                    MarkdownBlock::Heading { level: 1, .. },
                    MarkdownBlock::BlockQuote(blocks)
                ] if matches!(blocks.as_slice(), [MarkdownBlock::List { start: Some(1), items }]
                    if matches!(items[0].blocks.as_slice(), [MarkdownBlock::Paragraph(_), MarkdownBlock::List { start: None, .. }]))
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
            items[0].blocks[0],
            MarkdownBlock::Paragraph(vec![
                MarkdownInline::Strong(vec![MarkdownInline::Text("bold".into())]),
                MarkdownInline::Text(" and ".into()),
                MarkdownInline::Link {
                    url: "Page".into(),
                    content: vec![MarkdownInline::Text("alias".into())],
                },
            ])
        );
    }

    #[test]
    fn parses_task_list_markers() {
        let document = parse_markdown("- [ ] todo\n- [x] done\n- plain");
        let MarkdownBlock::List { items, .. } = &document.blocks[0] else {
            panic!("expected list, got {:#?}", document.blocks)
        };
        let tasks: Vec<_> = items.iter().map(|item| item.task).collect();
        assert_eq!(tasks, vec![Some(false), Some(true), None]);
        assert_eq!(
            items[0].blocks,
            vec![MarkdownBlock::Paragraph(vec![MarkdownInline::Text(
                "todo".into()
            )])]
        );
    }

    #[test]
    fn parses_tables() {
        let document = parse_markdown("| A | B |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |");
        let MarkdownBlock::Table { headers, rows } = &document.blocks[0] else {
            panic!("expected table, got {:#?}", document.blocks)
        };
        let plain = |cells: &[Vec<MarkdownInline>]| -> Vec<String> {
            cells
                .iter()
                .map(|cell| match cell.as_slice() {
                    [MarkdownInline::Text(text)] => text.clone(),
                    _ => format!("{cell:?}"),
                })
                .collect()
        };
        assert_eq!(plain(headers), ["A", "B"]);
        assert_eq!(rows.len(), 2);
        assert_eq!(plain(&rows[0]), ["1", "2"]);
        assert_eq!(plain(&rows[1]), ["3", "4"]);
    }
}
