//! Execution of compiled Base query plans.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use turso::Value;

use crate::document::base::{HARD_RESULT_LIMIT, SortDirection};
use crate::vault::catalog::{BaseDocument, BaseQuery, BaseSelection};

use super::CatalogDatabase;
use super::compiler::BaseQueryCompiler;
use super::turso_to_json;

const LINK_QUERY_BATCH: usize = 512;

impl CatalogDatabase {
    #[allow(clippy::too_many_lines)]
    pub(super) async fn query_base_on(
        &self,
        connection: &turso::Connection,
        query: BaseQuery,
    ) -> Result<BaseSelection> {
        let mut projection_compiler = BaseQueryCompiler::new(0);
        let mut projection_sql: BTreeMap<String, String> = BTreeMap::new();
        for source in &query.projections {
            if projection_sql.contains_key(source) {
                continue;
            }
            let sql = projection_compiler.compile_source(source, &query.formulas)?;
            projection_sql.insert(source.clone(), sql);
        }

        // Freeze now()/today(): their placeholders are empty Text payloads.
        let frozen = frozen_now();
        for value in &mut projection_compiler.parameters {
            if let Value::Text(payload) = value
                && payload.is_empty()
            {
                *value = Value::Text(frozen.clone());
            }
        }

        // Filter SQL for the row-returning SELECT; parameters start after the projections.
        let mut filter_compiler = BaseQueryCompiler::new(projection_compiler.parameters.len());
        let filter_sql = filter_compiler.compile_filter(&query.filters)?;
        for value in &mut filter_compiler.parameters {
            if let Value::Text(payload) = value
                && payload.is_empty()
            {
                *value = Value::Text(frozen.clone());
            }
        }

        // Filter SQL for aggregate statements (count, summaries); parameters start at one.
        let mut aggregate_filter_compiler = BaseQueryCompiler::new(0);
        let aggregate_filter_sql = aggregate_filter_compiler.compile_filter(&query.filters)?;
        for value in &mut aggregate_filter_compiler.parameters {
            if let Value::Text(payload) = value
                && payload.is_empty()
            {
                *value = Value::Text(frozen.clone());
            }
        }

        // Class SQL ride along as boolean SELECT columns; like filter SQL.
        let class_offset = projection_compiler
            .parameters
            .len()
            .saturating_add(filter_compiler.parameters.len());
        let mut class_compiler = BaseQueryCompiler::new(class_offset);
        let mut class_sql = Vec::with_capacity(query.classes.len());
        for class in &query.classes {
            class_sql.push(format!("({})", class_compiler.compile_filter(class)?));
        }
        for value in &mut class_compiler.parameters {
            if let Value::Text(payload) = value
                && payload.is_empty()
            {
                *value = Value::Text(frozen.clone());
            }
        }

        // Total matched rows ignore the display limit.
        let count_sql = format!("SELECT count(*) FROM documents WHERE ({aggregate_filter_sql})");
        let mut total_rows = connection
            .query(
                count_sql,
                turso::params_from_iter(aggregate_filter_compiler.parameters.clone()),
            )
            .await
            .context("count")?;
        let total_matched = total_rows
            .next()
            .await
            .context("count row")?
            .ok_or_else(|| anyhow::anyhow!("catalog returned no count row"))?
            .get::<i64>(0)
            .context("count cell")?;
        let total_matched = usize::try_from(total_matched).unwrap_or(usize::MAX);

        let mut columns = vec![
            "path".to_string(),
            "json(metadata)".to_string(),
            "created_ns".to_string(),
        ];
        let mut projection_alias: BTreeMap<String, String> = BTreeMap::new();
        for (index, source) in query.projections.iter().enumerate() {
            let sql = projection_sql
                .get(source)
                .ok_or_else(|| anyhow::anyhow!("projection {source:?} missing from plan"))?;
            columns.push(format!("{sql} AS \"c{index}\""));
            projection_alias.insert(source.clone(), format!("\"c{index}\""));
        }
        for (index, sql) in class_sql.iter().enumerate() {
            columns.push(format!("{sql} AS \"k{index}\""));
        }

        // Order by SELECT aliases;
        // repeating full expressions here would bind their parameters twice under positional placeholders.
        let mut order_terms = Vec::new();
        if let Some((group_source, direction)) = &query.group_by {
            let alias = projection_alias
                .get(group_source)
                .ok_or_else(|| anyhow::anyhow!("groupBy source missing from plan"))?;
            // The empty-key group renders last regardless of direction.
            order_terms.push(format!("{alias} IS NULL"));
            order_terms.push(format!("{alias} {}", direction_sql(*direction)));
        }
        for (source, direction) in &query.sort {
            let alias = projection_alias
                .get(source)
                .ok_or_else(|| anyhow::anyhow!("sort source missing from plan"))?;
            order_terms.push(format!("{alias} {}", direction_sql(*direction)));
        }
        order_terms.push("path".into());

        let limit = query
            .limit
            .unwrap_or(HARD_RESULT_LIMIT)
            .min(HARD_RESULT_LIMIT);
        // Turso does not accept bound parameters in LIMIT, so the ceiling is
        // inlined; it is always an integer within the hard result limit.
        let select_sql = format!(
            "SELECT {} FROM documents WHERE ({filter_sql}) ORDER BY {} LIMIT {limit}",
            columns.join(", "),
            order_terms.join(", ")
        );
        let mut row_params = projection_compiler.parameters.clone();
        row_params.extend(filter_compiler.parameters.clone());
        row_params.extend(class_compiler.parameters.clone());
        let mut rows = connection
            .query(select_sql, turso::params_from_iter(row_params))
            .await
            .context("main select")?;

        let projection_count = query.projections.len();
        let class_count = query.classes.len();
        let mut documents = Vec::new();
        while let Some(row) = rows.next().await.context("main row")? {
            // columns
            // 1=json
            let metadata = match row.get_value(1).context("metadata cell")? {
                Value::Null => None,
                Value::Text(json) => Some(serde_json::from_str(&json)?),
                value => bail!("Unexpected metadata value {value:?}"),
            };

            // >3=projections rename, c0...cN
            let mut values = Vec::with_capacity(projection_count);
            for index in 0..projection_count {
                values.push(turso_to_json(row.get_value(index.saturating_add(3))?));
            }

            // >3+N=class, k0...kN
            let mut class_hits = Vec::with_capacity(class_count);
            for index in 0..class_count {
                let column = projection_count.saturating_add(index).saturating_add(3);
                class_hits.push(matches!(
                    row.get_value(column)?,
                    Value::Integer(hit) if hit != 0
                ));
            }

            // 0=path
            let relative = PathBuf::from(row.get::<String>(0)?);

            documents.push(BaseDocument {
                path: self.root.join(relative),
                metadata,
                created_ns: row.get::<i64>(2)?,
                values,
                class_hits,
                links: Vec::new(),
            });
        }
        drop(rows);

        self.attach_outgoing_links(connection, &mut documents)
            .await
            .context("attach links")?;

        let summaries = self
            .compute_summaries(
                connection,
                &aggregate_filter_compiler,
                &aggregate_filter_sql,
                &query,
                &query.formulas,
            )
            .await?;

        Ok(BaseSelection {
            documents,
            total_matched,
            summaries,
        })
    }

    async fn attach_outgoing_links(
        &self,
        connection: &turso::Connection,
        documents: &mut [BaseDocument],
    ) -> Result<()> {
        if documents.is_empty() {
            return Ok(());
        }
        let selected_paths: Vec<String> = documents
            .iter()
            .map(|document| {
                document
                    .path
                    .strip_prefix(&self.root)
                    .unwrap_or(&document.path)
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        let mut index_by_path: HashMap<&str, usize> = HashMap::with_capacity(selected_paths.len());
        for (index, path) in selected_paths.iter().enumerate() {
            index_by_path.entry(path.as_str()).or_insert(index);
        }
        let mut attached: Vec<(usize, PathBuf)> = Vec::new();
        for batch in selected_paths.chunks(LINK_QUERY_BATCH) {
            let placeholders = batch.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!(
                "SELECT DISTINCT source_path, target_path FROM wiki_links \
                 WHERE target_path IS NOT NULL AND source_path IN ({placeholders})"
            );
            let params = batch
                .iter()
                .map(|path| Value::Text((*path).clone()))
                .collect::<Vec<_>>();
            let mut rows = connection
                .query(sql, turso::params_from_iter(params))
                .await?;
            while let Some(row) = rows.next().await? {
                let source: String = row.get::<String>(0)?.replace('\\', "/");
                let target: String = row.get::<String>(1)?;
                if let Some(&index) = index_by_path.get(source.as_str()) {
                    attached.push((index, self.root.join(target)));
                }
            }
        }
        for (index, target) in attached {
            if let Some(document) = documents.get_mut(index) {
                document.links.push(target);
            }
        }
        for document in documents.iter_mut() {
            document.links.sort();
            document.links.dedup();
        }
        Ok(())
    }
}

const fn direction_sql(direction: SortDirection) -> &'static str {
    match direction {
        SortDirection::Asc => "ASC",
        SortDirection::Desc => "DESC",
    }
}

/// Freezes `now()`/`today()` to one canonical ISO-8601 UTC timestamp per refresh,
/// matching the projection format used by file.mtime/file.ctime.
fn frozen_now() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_secs()).unwrap_or(0))
        .unwrap_or_default();
    time::OffsetDateTime::from_unix_timestamp(seconds).map_or_else(
        |_| "1970-01-01 00:00:00".to_string(),
        |datetime| {
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                datetime.year(),
                u8::from(datetime.month()),
                datetime.day(),
                datetime.hour(),
                datetime.minute(),
                datetime.second()
            )
        },
    )
}
