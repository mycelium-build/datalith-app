mod index;
pub mod picker;
mod query;

pub use index::Indexer;
pub const MIN_SEARCH_QUERY_LENGTH: usize = 3;

use std::path::PathBuf;

#[allow(dead_code)]
pub struct SearchEngine {
    pub indexer: Indexer,
}

impl SearchEngine {
    pub fn new(root: &PathBuf) -> tantivy::Result<Self> {
        let indexer = Indexer::new(root)?;
        Ok(Self { indexer })
    }
}
