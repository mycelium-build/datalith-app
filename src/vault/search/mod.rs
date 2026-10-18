pub mod index;
pub mod picker;
mod query;

pub use index::Indexer;
use query::build_query;

use std::ops::Range;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tantivy::snippet::SnippetGenerator;
use tantivy::{TantivyDocument, collector::TopDocs, schema::Value};

use crate::document::file_types::RegisteredFileTypes;

const MAX_SEARCH_RESULTS: usize = 25;
const SNIPPET_MAX_CHARS: usize = 140;

#[derive(Clone, Debug)]
pub struct SearchHit {
    pub path: PathBuf,
    pub score: f32,
    pub snippet: String,
    pub highlights: Vec<Range<usize>>,
}

pub struct SearchEngine {
    pub(crate) indexer: Indexer,
}

impl SearchEngine {
    pub(crate) fn open_existing(root: &Path, file_types: RegisteredFileTypes) -> Result<Self> {
        let indexer = Indexer::open_existing(root, file_types)?;
        Ok(Self { indexer })
    }

    #[must_use]
    pub(crate) fn search(&self, query_str: &str) -> Vec<SearchHit> {
        let Ok(reader) = self.indexer.index.reader() else {
            return Vec::new();
        };
        let searcher = reader.searcher();
        let query = build_query(
            query_str,
            &searcher,
            self.indexer.name_field,
            self.indexer.content_field,
        );
        let snippet_generator =
            SnippetGenerator::create(&searcher, query.as_ref(), self.indexer.content_field)
                .ok()
                .map(|mut generator| {
                    generator.set_max_num_chars(SNIPPET_MAX_CHARS);
                    generator
                });
        let Ok(top_docs) = searcher.search(
            &query,
            &TopDocs::with_limit(MAX_SEARCH_RESULTS).order_by_score(),
        ) else {
            return Vec::new();
        };

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            if let Ok(doc) = searcher.doc::<TantivyDocument>(doc_address)
                && let Some(path) = doc
                    .get_first(self.indexer.path_field)
                    .and_then(|v| v.as_str())
                    .map(PathBuf::from)
            {
                let content = doc
                    .get_first(self.indexer.content_field)
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let (snippet, highlights) = snippet_generator.as_ref().map_or_else(
                    || fallback_snippet(content),
                    |generator| snippet_from_doc(generator, &doc, content),
                );
                results.push(SearchHit {
                    path,
                    score,
                    snippet,
                    highlights,
                });
            }
        }
        results
    }
}

fn snippet_from_doc(
    generator: &SnippetGenerator,
    doc: &TantivyDocument,
    content: &str,
) -> (String, Vec<Range<usize>>) {
    let snippet = generator.snippet_from_doc(doc);
    let fragment = snippet.fragment();
    if fragment.is_empty() {
        return fallback_snippet(content);
    }

    let fragment_start = content.find(fragment).unwrap_or(0);
    let fragment_end = fragment_start.saturating_add(fragment.len());
    let prefix = if fragment_start > 0 { "…" } else { "" };
    let suffix = if fragment_end < content.len() {
        "…"
    } else {
        ""
    };
    let shift = prefix.len();

    let mut text = String::with_capacity(
        prefix
            .len()
            .saturating_add(fragment.len())
            .saturating_add(suffix.len()),
    );
    text.push_str(prefix);
    text.extend(fragment.chars().map(flatten_whitespace));
    text.push_str(suffix);

    let highlights = snippet
        .highlighted()
        .iter()
        .map(|range| range.start.saturating_add(shift)..range.end.saturating_add(shift))
        .collect();
    (text, highlights)
}

fn fallback_snippet(content: &str) -> (String, Vec<Range<usize>>) {
    let mut text: String = content
        .chars()
        .take(SNIPPET_MAX_CHARS)
        .map(flatten_whitespace)
        .collect();
    if text.chars().count() < content.chars().count() {
        text.push_str("…");
    }
    (text, Vec::new())
}

const fn flatten_whitespace(ch: char) -> char {
    match ch {
        '\n' | '\r' | '\t' => ' ',
        other => other,
    }
}
