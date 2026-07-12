pub(crate) mod index;
pub(crate) mod picker;
mod query;

pub(crate) use index::Indexer;
use query::build_query;

use std::path::PathBuf;

use anyhow::Result;
use tantivy::{TantivyDocument, collector::TopDocs, schema::Value};

use crate::consts::MAX_SEARCH_RESULTS;
use crate::file_types::RegisteredFileTypes;

pub(crate) struct SearchEngine {
    pub(crate) indexer: Indexer,
}

impl SearchEngine {
    pub(crate) fn new(root: &PathBuf, file_types: &RegisteredFileTypes) -> Result<Self> {
        let indexer = Indexer::new(root, file_types)?;
        Ok(Self { indexer })
    }

    #[must_use]
    pub(crate) fn search(&self, query_str: &str) -> Vec<PathBuf> {
        let reader = match self.indexer.index.reader() {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        let searcher = reader.searcher();
        let query = build_query(
            query_str,
            self.indexer.name_field,
            self.indexer.content_field,
        );
        let top_docs = match searcher.search(
            &query,
            &TopDocs::with_limit(MAX_SEARCH_RESULTS).order_by_score(),
        ) {
            Ok(docs) => docs,
            Err(_) => return Vec::new(),
        };

        let mut results = Vec::new();
        for (_score, doc_address) in top_docs {
            if let Ok(doc) = searcher.doc::<TantivyDocument>(doc_address) {
                if let Some(path) = doc
                    .get_first(self.indexer.path_field)
                    .and_then(|v| v.as_str())
                    .map(PathBuf::from)
                {
                    results.push(path);
                }
            }
        }
        results
    }
}
