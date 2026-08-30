use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use turso::Value;

use super::{
    Backlink, CatalogDatabase, FILE_BASENAME_SQL, FILE_NAME_SQL, PATH_WITHOUT_EXTENSION_SQL,
    escape_like_pattern, path_text,
};
use crate::vault::links;

impl CatalogDatabase {
    pub(crate) async fn resolve_path(&self, authored: &str) -> Result<Option<PathBuf>> {
        let connection = self.read_connection()?;
        Ok(resolve_path_on(&connection, authored)
            .await?
            .map(|path| self.root.join(path)))
    }

    pub(crate) async fn resolve_paths(
        &self,
        targets: BTreeSet<String>,
    ) -> Result<BTreeMap<String, Option<PathBuf>>> {
        let connection = self.read_connection()?;
        Ok(resolve_paths_on(&connection, targets)
            .await?
            .into_iter()
            .map(|(target, path)| (target, path.map(|path| self.root.join(path))))
            .collect())
    }

    pub(crate) async fn backlinks_under(&self, target: &Path) -> Result<Vec<Backlink>> {
        let relative_target = target
            .strip_prefix(&self.root)
            .context("Rename target is outside the Vault")?;
        let relative_target = relative_target.to_string_lossy().replace('\\', "/");
        let descendant_pattern = format!("{}/%", escape_like_pattern(&relative_target));
        let connection = self.read_connection()?;
        let mut rows = connection
            .query(
                "SELECT source_path, ordinal, target, target_path \
                 FROM wiki_links \
                 WHERE target_path = ? \
                    OR target_path LIKE ? ESCAPE '\\' \
                 ORDER BY source_path, ordinal",
                turso::params![relative_target, descendant_pattern],
            )
            .await?;
        let mut backlinks = Vec::new();
        while let Some(row) = rows.next().await? {
            backlinks.push(Backlink {
                source: PathBuf::from(row.get::<String>(0)?),
                ordinal: usize::try_from(row.get::<i64>(1)?)?,
                authored_target: row.get::<String>(2)?,
                target_path: PathBuf::from(row.get::<String>(3)?),
            });
        }
        Ok(backlinks)
    }
}

pub(super) async fn resolve_path_on(
    connection: &turso::Connection,
    authored: &str,
) -> Result<Option<PathBuf>> {
    let target = links::normalized_target(authored);
    if target.is_empty() {
        return Ok(None);
    }
    let sql = resolve_sql(
        target.contains('/'),
        Path::new(&target).extension().is_some(),
    );
    let mut rows = connection
        .query(sql, turso::params_from_iter(resolve_params(&target)))
        .await?;
    rows.next()
        .await?
        .map(|row| row.get::<String>(0).map(PathBuf::from))
        .transpose()
        .map_err(Into::into)
}

fn resolve_params(target: &str) -> Vec<Value> {
    let mut params = vec![Value::Text(target.to_string())];
    if target.contains('/') {
        params.push(Value::Text(format!("%/{}", escape_like_pattern(target))));
    }
    params
}

/// Resolves every normalized target with one compile per statement shape
/// (path/basename x extension), instead of one per target.
async fn resolve_paths_on(
    connection: &turso::Connection,
    targets: BTreeSet<String>,
) -> Result<BTreeMap<String, Option<PathBuf>>> {
    // Two authored spellings may normalize to the same catalog entry.
    let mut unique: BTreeMap<String, Option<PathBuf>> = targets
        .iter()
        .map(|authored| (links::normalized_target(authored), None))
        .filter(|(normalized, _)| !normalized.is_empty())
        .collect();

    let mut shapes: BTreeMap<(bool, bool), Vec<String>> = BTreeMap::new();
    for target in unique.keys() {
        shapes
            .entry((
                target.contains('/'),
                Path::new(target).extension().is_some(),
            ))
            .or_default()
            .push(target.clone());
    }
    for ((qualified, has_extension), bucket) in shapes {
        let mut statement = connection
            .prepare(resolve_sql(qualified, has_extension))
            .await?;
        for target in bucket {
            let mut rows = statement
                .query(turso::params_from_iter(resolve_params(&target)))
                .await?;
            let path = match rows.next().await? {
                Some(row) => Some(PathBuf::from(row.get::<String>(0)?)),
                None => None,
            };
            drop(rows);
            if let Some(entry) = unique.get_mut(&target) {
                *entry = path;
            }
        }
    }
    Ok(targets
        .into_iter()
        .map(|authored| {
            let normalized = links::normalized_target(&authored);
            let answer = if normalized.is_empty() {
                None
            } else {
                unique.get(&normalized).cloned().flatten()
            };
            (authored, answer)
        })
        .collect())
}

fn resolve_sql(qualified: bool, target_has_extension: bool) -> String {
    let compared_field = match (qualified, target_has_extension) {
        (true, true) => "path",
        (true, false) => PATH_WITHOUT_EXTENSION_SQL,
        (false, true) => FILE_BASENAME_SQL,
        (false, false) => FILE_NAME_SQL,
    };
    // A qualified target like `[[covers/x.png]]`  may live at any depth (`notes/covers/x.png`),
    // so accept exact paths first, then any path ending in the target.
    // Exact wins because the slash-count ordering below prefers paths with fewer segments.
    let predicate = if qualified {
        format!(
            "lower({compared_field}) = lower(?1) \
             OR {compared_field} LIKE ?2 ESCAPE '\\'"
        )
    } else {
        format!("lower({compared_field}) = lower(?1)")
    };
    format!(
        "SELECT path FROM documents \
         WHERE {predicate} \
         ORDER BY \
            length(path) - length(replace(path, '/', '')) ASC, \
            CASE WHEN lower(extension) = 'md' THEN 0 ELSE 1 END ASC, \
            lower(path) ASC, \
            path ASC \
         LIMIT 1"
    )
}

pub(super) async fn resolve_links(
    connection: &turso::Connection,
    targets: BTreeSet<String>,
) -> Result<()> {
    for target in targets {
        let resolved = resolve_path_on(connection, &target).await?;
        let resolved = resolved
            .as_deref()
            .map_or(Value::Null, |resolved| Value::Text(path_text(resolved)));
        connection
            .execute(
                "UPDATE wiki_links SET target_path = ? WHERE target = ? COLLATE NOCASE",
                turso::params![resolved, target],
            )
            .await?;
    }
    Ok(())
}

pub(super) fn link_target_candidates(path: &Path) -> BTreeSet<String> {
    let mut candidates = BTreeSet::new();
    candidates.insert(path_text(path));
    candidates.insert(path_text(&path.with_extension("")));
    if let Some(file_name) = path.file_name() {
        candidates.insert(file_name.to_string_lossy().into_owned());
    }
    if let Some(file_stem) = path.file_stem() {
        candidates.insert(file_stem.to_string_lossy().into_owned());
    }
    candidates
}

pub(super) async fn collect_matching_targets(
    connection: &turso::Connection,
    candidates: BTreeSet<String>,
    targets: &mut BTreeSet<String>,
) -> Result<()> {
    for candidate in candidates {
        let mut rows = connection
            .query(
                "SELECT DISTINCT target FROM wiki_links WHERE target = ? COLLATE NOCASE",
                [candidate],
            )
            .await?;
        while let Some(row) = rows.next().await? {
            targets.insert(row.get::<String>(0)?);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use turso::params;

    use super::*;

    #[test]
    fn resolves_links_from_catalogued_paths_using_ambiguity_order() {
        let root =
            std::env::temp_dir().join(format!("datalith-catalog-resolve-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        pollster::block_on(async {
            let database = CatalogDatabase::open(&root).await.unwrap();
            let connection = database.connection();
            for (path, extension, folder) in [
                ("Note.txt", "txt", ""),
                ("a/Note.md", "md", "a"),
                ("b/Other.txt", "txt", "b"),
                ("c/Other.md", "md", "c"),
                ("a/Same.md", "md", "a"),
                ("b/Same.md", "md", "b"),
            ] {
                connection
                    .execute(
                        "INSERT INTO documents(path, extension, folder, size_bytes, modified_ns, metadata) \
                         VALUES (?, ?, ?, 0, 0, NULL)",
                        params![path, extension, folder],
                    )
                    .await
                    .unwrap();
            }

            assert_eq!(
                database.resolve_path("Note").await.unwrap(),
                Some(root.join("Note.txt"))
            );
            assert_eq!(
                database.resolve_path("Other").await.unwrap(),
                Some(root.join("c/Other.md"))
            );
            assert_eq!(
                database.resolve_path("Same").await.unwrap(),
                Some(root.join("a/Same.md"))
            );
            assert_eq!(
                database.resolve_path("a/Same").await.unwrap(),
                Some(root.join("a/Same.md"))
            );
        });
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_qualified_targets_by_exact_path_then_suffix() {
        let root =
            std::env::temp_dir().join(format!("datalith-catalog-suffix-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        pollster::block_on(async {
            let database = CatalogDatabase::open(&root).await.unwrap();
            let connection = database.connection();
            for (path, extension, folder) in [
                (
                    "examples/bases/notes/covers/x.png",
                    "png",
                    "examples/bases/notes/covers",
                ),
                ("a/covers/x.png", "png", "a/covers"),
                ("nested/covZZers/x.png", "png", "nested"),
                ("docs/covers/pic.md", "md", "docs/covers"),
                ("deep/docs/covers/pic.md", "md", "deep/docs/covers"),
            ] {
                connection
                    .execute(
                        "INSERT INTO documents(path, extension, folder, size_bytes, modified_ns, metadata) \
                         VALUES (?, ?, ?, 0, 0, NULL)",
                        params![path, extension, folder],
                    )
                    .await
                    .unwrap();
            }

            // Suffix match resolves into the vault, fewest folders first.
            assert_eq!(
                database.resolve_path("covers/x.png").await.unwrap(),
                Some(root.join("a/covers/x.png"))
            );
            // Suffix matches are slash-anchored: `ers/x.png` must not match
            // inside the `covZZers` component.
            assert_eq!(database.resolve_path("ers/x.png").await.unwrap(), None);
            // LIKE metacharacters in the target are literal: `cov%ers` must
            // not match `covZZers` through a wildcard.
            assert_eq!(database.resolve_path("cov%ers/x.png").await.unwrap(), None);
            // Qualified targets without an extension match note paths.
            assert_eq!(
                database.resolve_path("covers/pic").await.unwrap(),
                Some(root.join("docs/covers/pic.md"))
            );
        });
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn backlink_descendant_query_treats_like_wildcards_as_path_text() {
        let root =
            std::env::temp_dir().join(format!("datalith-backlinks-like-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        pollster::block_on(async {
            let database = CatalogDatabase::open(&root).await.unwrap();
            let connection = database.connection();
            for (path, folder) in [
                ("Source.md", ""),
                ("Other.md", ""),
                ("%_/Target.md", "%_"),
                ("ab/Target.md", "ab"),
            ] {
                connection
                    .execute(
                        "INSERT INTO documents(path, extension, folder, size_bytes, modified_ns, metadata) \
                         VALUES (?, 'md', ?, 0, 0, NULL)",
                        params![path, folder],
                    )
                    .await
                    .unwrap();
            }
            connection
                .execute(
                    "INSERT INTO wiki_links(source_path, ordinal, target, target_path) \
                     VALUES ('Source.md', 0, '%_/Target', '%_/Target.md'), \
                            ('Other.md', 0, 'ab/Target', 'ab/Target.md')",
                    (),
                )
                .await
                .unwrap();

            let backlinks = database.backlinks_under(&root.join("%_")).await.unwrap();

            assert_eq!(backlinks.len(), 1);
            assert_eq!(backlinks[0].source, PathBuf::from("Source.md"));
            assert_eq!(backlinks[0].target_path, PathBuf::from("%_/Target.md"));
        });
        let _ = fs::remove_dir_all(root);
    }
}
