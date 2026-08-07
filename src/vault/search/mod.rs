pub mod index;
pub mod picker;
mod query;

pub use index::Indexer;
use query::build_query;

use std::path::{Path, PathBuf};

use anyhow::Result;
use tantivy::{TantivyDocument, collector::TopDocs, schema::Value};

use crate::document::file_types::RegisteredFileTypes;

const MAX_SEARCH_RESULTS: usize = 25;

pub struct SearchEngine {
    pub(crate) indexer: Indexer,
}

impl SearchEngine {
    pub(crate) fn open_existing(root: &Path, file_types: RegisteredFileTypes) -> Result<Self> {
        let indexer = Indexer::open_existing(root, file_types)?;
        Ok(Self { indexer })
    }

    #[must_use]
    pub(crate) fn search(&self, query_str: &str) -> Vec<PathBuf> {
        let Ok(reader) = self.indexer.index.reader() else {
            return Vec::new();
        };
        let searcher = reader.searcher();
        let query = build_query(
            query_str,
            self.indexer.name_field,
            self.indexer.content_field,
        );
        let Ok(top_docs) = searcher.search(
            &query,
            &TopDocs::with_limit(MAX_SEARCH_RESULTS).order_by_score(),
        ) else {
            return Vec::new();
        };

        let mut results = Vec::new();
        for (_score, doc_address) in top_docs {
            if let Ok(doc) = searcher.doc::<TantivyDocument>(doc_address)
                && let Some(path) = doc
                    .get_first(self.indexer.path_field)
                    .and_then(|v| v.as_str())
                    .map(PathBuf::from)
            {
                results.push(path);
            }
        }
        results
    }
}
