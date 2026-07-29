mod blockquote;
mod frontmatter;
mod links;
mod parse;

pub(crate) use self::frontmatter::{Frontmatter, FrontmatterValue};
pub(crate) use links::find_link_at_offset;

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

pub(crate) fn parse_markdown(text: &str) -> MarkdownDocument {
    let (frontmatter, body) = frontmatter::extract_frontmatter(text);
    let body = blockquote::normalize_blockquote_depth(&body);
    let body = links::convert_wiki_links(&body);

    let mut options = pulldown_cmark::Options::empty();
    options.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    options.insert(pulldown_cmark::Options::ENABLE_TABLES);

    let mut events = pulldown_cmark::Parser::new_ext(&body, options).peekable();
    MarkdownDocument {
        frontmatter: frontmatter.map(|content| frontmatter::parse_frontmatter(&content)),
        blocks: parse::parse_blocks(&mut events, None),
    }
}

#[cfg(test)]
mod tests {
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
}
