//! Wiki-link autocomplete queries against the catalog.

use anyhow::Result;

use super::CatalogDatabase;
use crate::vault::catalog::WikiLinkMatch;
use crate::vault::links::qualified_target;

impl CatalogDatabase {
    /// Returns up to `limit` catalog documents matching a wiki-link query.
    ///
    /// `query` must already be lowercased.
    /// `label_key` and `path_key` are folded with Rust's Unicode lowercasing when a document is written.
    ///
    /// `path` is the only qualified identifier returned here.
    /// A minimal target is resolved separately when the user accepts a suggestion.
    pub(crate) async fn suggest_wiki_links(
        &self,
        query: &str,
        excluded: Option<&str>,
        limit: usize,
    ) -> Result<Vec<WikiLinkMatch>> {
        // `matched` ranks matches by what the user most likely means:
        //   0 = label starts with the query
        //   1 = label contains the query
        //   2 = path contains the query
        // Ties prefer shorter labels, then label and path order.
        //
        // `|`, `#`, `[`, and `]` are wiki-link syntax,
        // so paths containing them cannot be safely inserted.
        //
        // Parameters: ?1 lowercased query, ?2 excluded current path, ?3 limit.
        let sql = "SELECT path, label, NULLIF(folder, '') AS detail \
                   FROM documents \
                   WHERE (?2 IS NULL OR path <> ?2) \
                     AND instr(path, '|') = 0 AND instr(path, '#') = 0 \
                     AND instr(path, '[') = 0 AND instr(path, ']') = 0 \
                     AND (instr(label_key, ?1) > 0 OR instr(path_key, ?1) > 0) \
                   ORDER BY \
                       CASE \
                           WHEN instr(label_key, ?1) = 1 THEN 0 \
                           WHEN instr(label_key, ?1) > 0 THEN 1 \
                           ELSE 2 \
                       END, \
                       length(label), label_key, path \
                   LIMIT ?3";
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let excluded = excluded.map(str::to_string);
        let connection = self.connection().await?;
        let mut rows = connection
            .query(sql, turso::params![query, excluded, limit])
            .await?;
        let mut matches = Vec::new();
        while let Some(row) = rows.next().await? {
            matches.push(WikiLinkMatch {
                path: row.get::<String>(0)?,
                label: row.get::<String>(1)?,
                detail: row.get::<Option<String>>(2)?,
            });
        }
        Ok(matches)
    }

    /// Resolves the target to insert for an accepted suggestion.
    ///
    /// A unique file name can stay unqualified.
    /// If another document shares the label,
    /// the full relative path is used instead.
    /// This runs once per accepted suggestion.
    pub(crate) async fn wiki_link_target(&self, path: &str, label: &str) -> Result<String> {
        let duplicate = self.has_duplicate_label(path, label).await?;
        Ok(if duplicate {
            qualified_target(path)
        } else {
            label.to_string()
        })
    }

    async fn has_duplicate_label(&self, path: &str, label: &str) -> Result<bool> {
        let sql = "SELECT 1 FROM documents \
                   WHERE path <> ?1 \
                     AND label_key = ?2 \
                   LIMIT 1";
        let connection = self.connection().await?;
        let mut rows = connection
            .query(sql, turso::params![path, label.to_lowercase()])
            .await?;
        Ok(rows.next().await?.is_some())
    }
}
