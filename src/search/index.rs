use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use tantivy::{
    self, DocAddress, Index, IndexWriter, TantivyDocument, Term, doc,
    schema::{Field, Value},
};

pub fn file_fingerprint(path: &Path) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    if let Ok(meta) = fs::metadata(path) {
        meta.len().hash(&mut hasher);
        if let Ok(mtime) = meta.modified()
            && let Ok(dur) = mtime.duration_since(std::time::UNIX_EPOCH)
        {
            dur.as_secs().hash(&mut hasher);
        }
    }
    hasher.finish()
}

pub fn is_indexable(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("txt" | "md")
    )
}

pub(crate) fn walk_indexable_files(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with('.') {
                    continue;
                }
                if path.is_dir() {
                    stack.push(path);
                } else if is_indexable(&path) {
                    paths.push(path);
                }
            }
        }
    }
    paths
}

pub fn index_files(
    writer: &mut IndexWriter,
    dir: &Path,
    path_field: Field,
    name_field: Field,
    content_field: Field,
    fingerprint_field: Field,
) -> tantivy::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                index_files(
                    writer,
                    &path,
                    path_field,
                    name_field,
                    content_field,
                    fingerprint_field,
                )?;
            } else {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if ext == "txt" || ext == "md" {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    let content = fs::read_to_string(&path).unwrap_or_default();
                    let fp = file_fingerprint(&path);
                    writer.add_document(doc!(
                        path_field => path.to_string_lossy().as_ref(),
                        name_field => name,
                        content_field => content.as_str(),
                        fingerprint_field => fp.to_string(),
                    ))?;
                }
            }
        }
    }
    Ok(())
}

pub fn incremental_update(
    index: &Index,
    root: &Path,
    path_field: Field,
    name_field: Field,
    content_field: Field,
    fingerprint_field: Field,
) -> tantivy::Result<()> {
    let reader = index.reader()?;
    let searcher = reader.searcher();

    let mut current: HashMap<PathBuf, u64> = HashMap::new();
    collect_files(root, &mut current);

    let mut writer = index.writer(50_000_000)?;

    for (segment_ord, seg_reader) in searcher.segment_readers().iter().enumerate() {
        let segment_ord = segment_ord as u32;
        for doc_id in 0..seg_reader.max_doc() {
            if seg_reader.is_deleted(doc_id) {
                continue;
            }
            let addr = DocAddress::new(segment_ord, doc_id);
            let doc: TantivyDocument = match searcher.doc(addr) {
                Ok(d) => d,
                Err(_) => continue,
            };
            let path_str = doc.get_first(path_field).and_then(|v| v.as_str());
            let old_fp = doc
                .get_first(fingerprint_field)
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<u64>().ok());

            let Some((path_str, old_fp)) = path_str.zip(old_fp) else {
                continue;
            };
            let path = PathBuf::from(path_str);

            match current.remove(&path) {
                Some(new_fp) if new_fp == old_fp => {}
                _ => {
                    println!("Update: {}", path_str);
                    writer.delete_term(Term::from_field_text(path_field, path_str));
                }
            }
        }
    }

    drop(searcher);
    drop(reader);

    add_files(
        &mut writer,
        &current,
        path_field,
        name_field,
        content_field,
        fingerprint_field,
    )?;

    writer.commit()?;
    Ok(())
}

pub fn add_files(
    writer: &mut IndexWriter,
    files: &HashMap<PathBuf, u64>,
    path_field: Field,
    name_field: Field,
    content_field: Field,
    fingerprint_field: Field,
) -> tantivy::Result<()> {
    for (path, fp) in files {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let content = fs::read_to_string(path).unwrap_or_default();
        writer.add_document(doc!(
            path_field => path.to_string_lossy().as_ref(),
            name_field => name,
            content_field => content.as_str(),
            fingerprint_field => fp.to_string(),
        ))?;
    }
    Ok(())
}

fn collect_files(dir: &Path, files: &mut HashMap<PathBuf, u64>) {
    for path in walk_indexable_files(dir) {
        files.insert(path.clone(), file_fingerprint(&path));
    }
}
