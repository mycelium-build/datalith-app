mod index;
pub mod picker;
mod query;

use index::{add_files, file_fingerprint, incremental_update, index_files, is_indexable};
use query::build_query;

pub const MAX_SEARCH_RESULTS: usize = 25;
pub const MIN_SEARCH_QUERY_LENGTH: usize = 3;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use tantivy::{self, DocAddress, Index, IndexWriter, TantivyDocument, Term, collector::TopDocs, schema::*};

#[derive(Clone)]
pub struct SearchResult {
    pub path: PathBuf,
}

#[allow(dead_code)]
pub struct SearchEngine {
    index: Index,
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

        Ok(Self {
            index,
            path_field,
            name_field,
            content_field,
            fingerprint_field,
        })
    }

    pub fn search(&self, query_str: &str) -> Vec<SearchResult> {
        let reader = match self.index.reader() {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        let searcher = reader.searcher();
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
                    results.push(SearchResult { path });
                }
            }
        }
        results
    }

    pub fn add_file(&self, path: &Path) -> tantivy::Result<()> {
        if !is_indexable(path) || !path.is_file() {
            return Ok(());
        }
        let mut files = HashMap::new();
        files.insert(path.to_path_buf(), file_fingerprint(path));
        let mut writer: IndexWriter<TantivyDocument> = self.index.writer(50_000_000)?;
        add_files(
            &mut writer,
            &files,
            self.path_field,
            self.name_field,
            self.content_field,
            self.fingerprint_field,
        )?;
        writer.commit()?;
        Ok(())
    }

    pub fn remove_file(&self, path: &Path) -> tantivy::Result<()> {
        let path_str = path.to_string_lossy();
        let mut writer: IndexWriter<TantivyDocument> = self.index.writer(50_000_000)?;
        writer.delete_term(Term::from_field_text(self.path_field, &path_str));
        writer.commit()?;
        Ok(())
    }

    pub fn rename_file(&self, old_path: &Path, new_path: &Path) -> tantivy::Result<()> {
        self.remove_file(old_path)?;
        self.add_file(new_path)
    }

    pub fn all_paths(&self) -> Vec<PathBuf> {
        let reader = match self.index.reader() {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        let searcher = reader.searcher();
        let mut paths = Vec::new();
        for (segment_ord, seg_reader) in searcher.segment_readers().iter().enumerate() {
            let segment_ord = segment_ord as u32;
            for doc_id in 0..seg_reader.max_doc() {
                if seg_reader.is_deleted(doc_id) {
                    continue;
                }
                let addr = DocAddress::new(segment_ord, doc_id);
                if let Ok(doc) = searcher.doc::<TantivyDocument>(addr) {
                    if let Some(path_str) = doc.get_first(self.path_field).and_then(|v| v.as_str()) {
                        paths.push(PathBuf::from(path_str));
                    }
                }
            }
        }
        paths
    }
}
