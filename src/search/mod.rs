mod index;
pub mod picker;
mod query;

use index::{incremental_update, index_files};
use query::build_query;

const MAX_SEARCH_RESULTS: usize = 25;

use std::fs;
use std::path::{Path, PathBuf};

use tantivy::{self, Index, IndexReader, TantivyDocument, collector::TopDocs, schema::*};

#[derive(Clone)]
pub struct SearchResult {
    pub path: PathBuf,
    pub snippet: String,
}

#[allow(dead_code)]
pub struct SearchEngine {
    index: Index,
    reader: IndexReader,
    path_field: Field,
    name_field: Field,
    content_field: Field,
    fingerprint_field: Field,
}

impl SearchEngine {
    pub fn new(root: &Path) -> tantivy::Result<Self> {
        let index_path = root.join(".datalith").join("search_index");

        let mut schema_builder = Schema::builder();
        let path_field = schema_builder.add_text_field("path", STRING | STORED);
        let name_field = schema_builder.add_text_field("name", TEXT | STORED);
        let content_field = schema_builder.add_text_field("content", TEXT | STORED);
        let fingerprint_field = schema_builder.add_text_field("fingerprint", STRING | STORED);
        let schema = schema_builder.build();

        let (index, needs_build) = if index_path.exists() {
            (Index::open_in_dir(&index_path)?, false)
        } else {
            fs::create_dir_all(&index_path).map_err(|e| {
                tantivy::TantivyError::InvalidArgument(format!("Failed to create index dir: {e}"))
            })?;
            (Index::create_in_dir(&index_path, schema)?, true)
        };

        if needs_build {
            let mut writer = index.writer(50_000_000)?;
            index_files(
                &mut writer,
                root,
                path_field,
                name_field,
                content_field,
                fingerprint_field,
            )?;
            writer.commit()?;
        } else {
            incremental_update(
                &index,
                root,
                path_field,
                name_field,
                content_field,
                fingerprint_field,
            )?;
        }

        let reader = index.reader()?;

        Ok(Self {
            index,
            reader,
            path_field,
            name_field,
            content_field,
            fingerprint_field,
        })
    }

    pub fn search(&self, query_str: &str) -> Vec<SearchResult> {
        let searcher = self.reader.searcher();
        let query = build_query(query_str, self.name_field, self.content_field);
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
                let path = doc
                    .get_first(self.path_field)
                    .and_then(|v| v.as_str())
                    .map(PathBuf::from);

                if let Some(path) = path {
                    let snippet = tantivy::snippet::SnippetGenerator::create(
                        &searcher,
                        &query,
                        self.content_field,
                    )
                    .ok()
                    .map(|generator| {
                        let snip = generator.snippet_from_doc(&doc);
                        snip.fragment().to_string()
                    })
                    .unwrap_or_default();

                    results.push(SearchResult { path, snippet });
                }
            }
        }
        results
    }
}
