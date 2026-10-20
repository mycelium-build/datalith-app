//! Wiki-link autocomplete queries against the catalog.

use anyhow::Result;

use super::{CatalogDatabase, FILE_BASENAME_SQL, FILE_NAME_SQL};
use crate::vault::catalog::WikiLinkMatch;
use crate::vault::links::qualified_target;

impl CatalogDatabase {
    /// Returns up to `limit` catalog documents matching a wiki-link query.
    ///
    /// Matching, ranking, and limiting happen in `Turso`.
    ///
    /// `path` is the only qualified identifier returned here.
    /// A minimal target is resolved separately when the user accepts a suggestion.
    pub(crate) async fn suggest_wiki_links(
        &self,
        query: &str,
        excluded: Option<&str>,
        limit: usize,
    ) -> Result<Vec<WikiLinkMatch>> {
        // Markdown files display without `.md`;
        // every other file keeps its extension.
        // `path` itself is returned unchanged.
        let label = format!(
            "CASE WHEN lower(extension) = 'md' \
             THEN {FILE_NAME_SQL} \
             ELSE {FILE_BASENAME_SQL} END"
        );

        // `computed` derives the display label once.
        // `keyed` lowercases the label and path once for matching and sorting.
        //
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
        let sql = format!(
            "WITH computed AS (\
                 SELECT path, folder, {label} AS label \
                 FROM documents \
                 WHERE (?2 IS NULL OR path <> ?2) \
                   AND instr(path, '|') = 0 AND instr(path, '#') = 0 \
                   AND instr(path, '[') = 0 AND instr(path, ']') = 0\
             ), keyed AS (\
                 SELECT path, folder, label, \
                        lower(label) AS label_key, \
                        lower(path) AS path_key \
                 FROM computed\
             ) \
             SELECT path, label, NULLIF(folder, '') AS detail \
             FROM keyed \
             WHERE instr(label_key, ?1) > 0 \
                OR instr(path_key, ?1) > 0 \
             ORDER BY \
                 CASE \
                     WHEN instr(label_key, ?1) = 1 THEN 0 \
                     WHEN instr(label_key, ?1) > 0 THEN 1 \
                     ELSE 2 \
                 END, \
                 length(label), label_key, path \
             LIMIT ?3"
        );
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let excluded = excluded.map(str::to_string);
        let connection = self.connection().await?;
        let mut rows = connection
            .query(&sql, turso::params![query, excluded, limit])
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
        let label_sql = format!(
            "CASE WHEN lower(extension) = 'md' \
             THEN {FILE_NAME_SQL} \
             ELSE {FILE_BASENAME_SQL} END"
        );
        let sql = format!(
            "SELECT 1 FROM documents \
             WHERE path <> ?1 \
               AND lower({label_sql}) = ?2 \
             LIMIT 1"
        );
        let connection = self.connection().await?;
        let mut rows = connection
            .query(&sql, turso::params![path, label.to_lowercase()])
            .await?;
        Ok(rows.next().await?.is_some())
    }
}
