mod index;
pub mod picker;
mod query;

pub use index::Indexer;
use query::build_query;

pub const MAX_SEARCH_RESULTS: usize = 25;
pub const MIN_SEARCH_QUERY_LENGTH: usize = 3;

use std::path::PathBuf;

use tantivy::{TantivyDocument, collector::TopDocs, schema::Value};

#[allow(dead_code)]
pub struct SearchEngine {
    pub indexer: Indexer,
}

impl SearchEngine {
    pub fn new(root: &PathBuf) -> tantivy::Result<Self> {
        let indexer = Indexer::new(root)?;
        Ok(Self { indexer })
    }

    pub fn search(&self, query_str: &str) -> Vec<PathBuf> {
        let reader = match self.indexer.index().reader() {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        let searcher = reader.searcher();
        let query = build_query(query_str, self.indexer.name_field(), self.indexer.content_field());
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
                    .get_first(self.indexer.path_field())
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
