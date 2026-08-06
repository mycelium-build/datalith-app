use std::collections::BTreeSet;
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
        let connection = self.connection();
        Ok(resolve_path_on(&connection, authored)
            .await?
            .map(|path| self.root.join(path)))
    }

    pub(crate) async fn backlinks_under(&self, target: &Path) -> Result<Vec<Backlink>> {
        let relative_target = target
            .strip_prefix(&self.root)
            .context("Rename target is outside the Vault")?;
        let relative_target = relative_target.to_string_lossy().replace('\\', "/");
        let descendant_pattern = format!("{}/%", escape_like_pattern(&relative_target));
        let connection = self.connection();
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
                ordinal: row.get::<i64>(1)? as usize,
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
    let qualified = target.contains('/');
    let target_has_extension = Path::new(&target).extension().is_some();
    let compared_field = match (qualified, target_has_extension) {
        (true, true) => "path",
        (true, false) => PATH_WITHOUT_EXTENSION_SQL,
        (false, true) => FILE_BASENAME_SQL,
        (false, false) => FILE_NAME_SQL,
    };
    let sql = format!(
        "SELECT path FROM documents \
         WHERE lower({compared_field}) = lower(?) \
         ORDER BY \
            length(path) - length(replace(path, '/', '')) ASC, \
            CASE WHEN lower(extension) = 'md' THEN 0 ELSE 1 END ASC, \
            lower(path) ASC, \
            path ASC \
         LIMIT 1"
    );
    let mut rows = connection.query(sql, [target]).await?;
    rows.next()
        .await?
        .map(|row| row.get::<String>(0).map(PathBuf::from))
        .transpose()
        .map_err(Into::into)
}

pub(super) async fn resolve_links(
    connection: &turso::Connection,
    targets: BTreeSet<String>,
) -> Result<()> {
    for target in targets {
        let resolved = resolve_path_on(connection, &target).await?;
        let resolved = resolved
            .as_deref()
            .map(path_text)
            .map(Value::Text)
            .unwrap_or(Value::Null);
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
