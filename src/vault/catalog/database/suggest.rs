//! Wiki-link autocomplete queries against the catalog.

use anyhow::Result;

use super::{CatalogDatabase, FILE_BASENAME_SQL, FILE_NAME_SQL};
use crate::vault::catalog::WikiLinkMatch;

impl CatalogDatabase {
    /// Returns up to `limit` catalog documents whose path can be inserted as a `[[wiki link]]` target.
    ///
    /// Matching, ranking, and limiting all happen in `SQLite`
    /// so the editor never keeps the full vault file list in memory.
    /// The query is built around three steps:
    ///
    /// 1. `computed` derives the two forms a suggestion can insert.
    /// 2. `matched` filters by substring, excludes unusable paths, and ranks rows.
    /// 3. `name_counts` detects duplicate file names so `folder/Note` is used instead of an ambiguous `Note`.
    pub(crate) async fn suggest_wiki_links(
        &self,
        query: &str,
        excluded: Option<&str>,
        limit: usize,
    ) -> Result<Vec<WikiLinkMatch>> {
        // Wiki links to Markdown files omit the extension (`[[Note]]`),
        // while links to other files keep it (`[[image.png]]`).
        // `label` is the name shown in the popup and `qualified` is the vault-relative target;
        // both use that same link-target shape so what is displayed is what is inserted.
        //
        // `FILE_NAME_SQL` strips any extension.
        // `FILE_BASENAME_SQL` keeps the extension for other files.
        let label = format!(
            "CASE WHEN lower(extension) = 'md' \
             THEN {FILE_NAME_SQL} \
             ELSE {FILE_BASENAME_SQL} END"
        );

        // `computed` also produces `qualified`, the full relative path inserted into the link.
        // Markdown files lose their three-character `.md` extension;
        // other files keep their whole path.
        //
        // `matched` ranks matches by what the user most likely means:
        //   0 = label starts with the query
        //   1 = qualified path starts with the query
        //   2 = label contains the query
        //   3 = qualified path contains the query
        //   4 = full path contains the query
        // Ties prefer the shortest matching value, then name and path order.
        //
        // `|`, `#`, `[`, and `]` are wiki-link syntax (aliases, anchors, and delimiters).
        // A path containing one cannot be represented safely in `[[...]]`,
        // so those rows are filtered.
        //
        // `name_counts` counts labels across the whole vault, not just the current matches.
        // If `Note` exists in several folders, every match is marked ambiguous even
        // when the query only matched one of them; the UI then inserts `folder/Note` instead of `Note`.
        // A full relative path is the `documents` primary key, so it has no further ambiguity.
        //
        // Parameters: ?1 lowercased query, ?2 excluded current path, ?3 limit.
        let sql = format!(
            "WITH computed AS (\
                 SELECT path, folder, {label} AS label, \
                        CASE WHEN lower(extension) = 'md' \
                             THEN substr(path, 1, length(path) - 3) \
                             ELSE path END AS qualified \
                 FROM documents\
             ), matched AS (\
                 SELECT path, label, qualified, \
                        CASE \
                            WHEN instr(lower(label), ?1) = 1 THEN 0 \
                            WHEN instr(lower(qualified), ?1) = 1 THEN 1 \
                            WHEN instr(lower(label), ?1) > 0 THEN 2 \
                            WHEN instr(lower(qualified), ?1) > 0 THEN 3 \
                            ELSE 4 \
                        END AS rank, \
                        CASE \
                            WHEN instr(lower(label), ?1) = 1 THEN length(label) \
                            WHEN instr(lower(qualified), ?1) = 1 THEN length(qualified) \
                            WHEN instr(lower(label), ?1) > 0 THEN length(label) \
                            WHEN instr(lower(qualified), ?1) > 0 THEN length(qualified) \
                            ELSE length(path) \
                        END AS match_length \
                 FROM computed \
                 WHERE (?2 IS NULL OR path <> ?2) \
                   AND instr(path, '|') = 0 AND instr(path, '#') = 0 \
                   AND instr(path, '[') = 0 AND instr(path, ']') = 0 \
                   AND (instr(lower(label), ?1) > 0 \
                     OR instr(lower(qualified), ?1) > 0 \
                     OR instr(lower(path), ?1) > 0)\
             ), name_counts AS (\
                 SELECT lower(label) AS name_key, count(*) AS name_count \
                 FROM computed \
                 GROUP BY lower(label)\
             ) \
             SELECT matched.path, name_counts.name_count \
             FROM matched \
             LEFT JOIN name_counts ON name_counts.name_key = lower(matched.label) \
             ORDER BY matched.rank, matched.match_length, \
                      lower(matched.label), lower(matched.qualified), matched.path \
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
                path: self.root.join(row.get::<String>(0)?),
                ambiguous: row.get::<i64>(1)? > 1,
            });
        }
        Ok(matches)
    }
}
