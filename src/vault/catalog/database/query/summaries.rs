//! Summary compilation and execution for Base queries.

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use turso::Value;

use crate::document::base::{AggregateFn, CustomSummary, PostFn, Summary};
use crate::document::expr::Expr;
use crate::vault::catalog::{BaseQuery, SummaryRef};

use super::CatalogDatabase;
use super::compiler::BaseQueryCompiler;
use super::helpers::{parenthesize, turso_to_json};

pub(super) enum SummaryComputation {
    OutOfSql, // like median, it reserves the slot so results stay aligned with `query.summaries`
    /// A plain aggregate expression evaluated in the shared SELECT.
    Sql(String),
}

impl CatalogDatabase {
    pub(super) async fn compute_summaries(
        &self,
        connection: &turso::Connection,
        filter_compiler: &BaseQueryCompiler,
        filter_sql: &str,
        query: &BaseQuery,
        formulas: &BTreeMap<String, Expr>,
    ) -> Result<Vec<serde_json::Value>> {
        if query.summaries.is_empty() {
            return Ok(Vec::new());
        }
        let mut params = filter_compiler.parameters.clone();
        let mut computations: Vec<SummaryComputation> = Vec::new();
        let mut medians: Vec<(usize, String)> = Vec::new();
        let mut next_offset = filter_compiler.parameters.len();
        for (source, reference) in &query.summaries {
            let mut property_compiler = BaseQueryCompiler::new(next_offset);
            let property_sql = property_compiler.compile_source(source, formulas)?;
            next_offset = next_offset.saturating_add(property_compiler.parameters.len());
            params.extend(property_compiler.parameters.clone());
            if matches!(reference, SummaryRef::Default(Summary::Median)) {
                medians.push((computations.len(), source.clone()));
                computations.push(SummaryComputation::OutOfSql);
                continue;
            }
            computations.push(match reference {
                SummaryRef::Default(summary) => {
                    default_summary_computation(*summary, &property_sql)
                }
                SummaryRef::Custom(custom) => custom_summary_computation(custom, &property_sql),
            });
        }

        let mut results = vec![serde_json::Value::Null; query.summaries.len()];
        let aggregate_slots: Vec<usize> = computations
            .iter()
            .enumerate()
            .filter(|(_, computation)| !matches!(computation, SummaryComputation::OutOfSql))
            .map(|(index, _)| index)
            .collect();
        if !aggregate_slots.is_empty() {
            let select_list = aggregate_slots
                .iter()
                .enumerate()
                .filter_map(|(position, index)| {
                    let _ = position;
                    match computations.get(*index) {
                        Some(SummaryComputation::Sql(sql)) => Some(sql.clone()),
                        _ => None,
                    }
                })
                .collect::<Vec<_>>();
            let sql = format!(
                "SELECT {} FROM documents WHERE ({filter_sql})",
                select_list.join(", ")
            );
            let mut rows = connection
                .query(sql, turso::params_from_iter(params))
                .await
                .context("summary select")?;
            if let Some(row) = rows.next().await.context("summary row")? {
                for (position, index) in aggregate_slots.iter().enumerate() {
                    let value = turso_to_json(row.get_value(position)?);
                    if let Some(slot) = results.get_mut(*index) {
                        *slot = value;
                    }
                }
            }
        }
        for (index, source) in &medians {
            if let Some(slot) = results.get_mut(*index) {
                *slot = self
                    .compute_median(connection, filter_compiler, filter_sql, formulas, source)
                    .await?;
            }
        }
        Ok(results)
    }

    async fn compute_median(
        &self,
        connection: &turso::Connection,
        filter_compiler: &BaseQueryCompiler,
        filter_sql: &str,
        formulas: &BTreeMap<String, Expr>,
        source: &str,
    ) -> Result<serde_json::Value> {
        let mut property_compiler = BaseQueryCompiler::new(filter_compiler.parameters.len());
        let property_sql = property_compiler.compile_source(source, formulas)?;
        let count_sql = format!(
            "SELECT count(*) FROM documents WHERE ({filter_sql}) AND ({property_sql}) IS NOT NULL"
        );
        let rows_params: Vec<Value> = filter_compiler
            .parameters
            .iter()
            .chain(property_compiler.parameters.iter())
            .cloned()
            .collect();
        let mut rows = connection
            .query(count_sql, turso::params_from_iter(rows_params))
            .await
            .context("median count")?;
        let count = rows
            .next()
            .await
            .context("median count row")?
            .ok_or_else(|| anyhow::anyhow!("median count query returned no row"))?
            .get::<i64>(0)
            .context("median count cell")?;
        drop(rows);
        if count == 0 {
            return Ok(serde_json::Value::Null);
        }
        let offset = median_lower_offset(count);
        // Even counts need the middle pair to average;
        // odd counts need the exact middle value alone.
        let take = median_window_size(count);
        let window_sql = format!(
            "SELECT ({property_sql}) FROM documents WHERE ({filter_sql}) \
             AND ({property_sql}) IS NOT NULL ORDER BY ({property_sql}) \
             LIMIT {take} OFFSET {offset}"
        );
        let params: Vec<Value> = filter_compiler
            .parameters
            .iter()
            .chain(property_compiler.parameters.iter())
            .cloned()
            .collect();
        let mut rows = connection
            .query(window_sql, turso::params_from_iter(params))
            .await
            .context("median window")?;
        let lower = rows
            .next()
            .await
            .context("median row")?
            .ok_or_else(|| anyhow::anyhow!("median window query returned no row"))?
            .get_value(0)
            .context("median cell")?;
        let upper = rows
            .next()
            .await
            .context("median second row")?
            .map(|row| row.get_value(0).context("median second cell"))
            .transpose()?;
        drop(rows);
        let lower = turso_to_json(lower);
        match upper.map(turso_to_json) {
            Some(upper) => Ok(median_average(&lower, &upper)),
            None => Ok(lower),
        }
    }
}

#[allow(clippy::manual_midpoint)]
const fn median_lower_offset(count: i64) -> i64 {
    count.saturating_sub(1) / 2
}

const fn median_window_size(count: i64) -> i64 {
    if count % 2 == 0 { 2 } else { 1 }
}

fn median_average(lower: &serde_json::Value, upper: &serde_json::Value) -> serde_json::Value {
    match (lower.as_f64(), upper.as_f64()) {
        (Some(lower), Some(upper)) => serde_json::Value::from(f64::midpoint(lower, upper)),
        _ if lower == upper => lower.clone(),
        // Non-numeric middles have no defined mean.
        _ => serde_json::Value::Null,
    }
}

fn default_summary_computation(summary: Summary, property_sql: &str) -> SummaryComputation {
    let column = parenthesize(property_sql);
    match summary {
        Summary::Average => SummaryComputation::Sql(format!("avg({column})")),
        Summary::Min | Summary::Earliest => SummaryComputation::Sql(format!("min({column})")),
        Summary::Max | Summary::Latest => SummaryComputation::Sql(format!("max({column})")),
        Summary::Sum => SummaryComputation::Sql(format!("sum({column})")),
        Summary::Range => SummaryComputation::Sql(format!("(max({column}) - min({column}))")),
        Summary::DateRange => SummaryComputation::Sql(format!(
            "(julianday(max({column})) - julianday(min({column})))"
        )),
        // Medians are detected before this call and never reach here.
        Summary::Median => SummaryComputation::Sql("0".into()),
        // Turso's sqrt returns NULL for negative inputs (round-off below zero).
        Summary::Stddev => SummaryComputation::Sql(format!(
            "sqrt(avg({column} * {column}) - avg({column}) * avg({column}))"
        )),
        Summary::Checked => SummaryComputation::Sql(format!(
            "sum(CASE WHEN COALESCE({column}, 0) <> 0 THEN 1 ELSE 0 END)"
        )),
        Summary::Unchecked => SummaryComputation::Sql(format!(
            "sum(CASE WHEN {column} IS NOT NULL AND COALESCE({column}, 0) = 0 THEN 1 ELSE 0 END)"
        )),
        Summary::EmptyCount => SummaryComputation::Sql(format!("(count(*) - count({column}))")),
        Summary::FilledCount => SummaryComputation::Sql(format!("count({column})")),
        Summary::Unique => SummaryComputation::Sql(format!("count(DISTINCT {column})")),
    }
}

fn custom_summary_computation(custom: &CustomSummary, property_sql: &str) -> SummaryComputation {
    let column = parenthesize(property_sql);
    let aggregate = match custom.aggregate {
        AggregateFn::Mean => "avg",
        AggregateFn::Min => "min",
        AggregateFn::Max => "max",
        AggregateFn::Sum => "sum",
        AggregateFn::Count => "count",
    };
    let mut sql = format!("{aggregate}({column})");
    for post in &custom.post {
        match post {
            PostFn::Round(digits) => sql = format!("round({sql}, {digits})"),
            PostFn::Abs => sql = format!("abs({sql})"),
        }
    }
    SummaryComputation::Sql(sql)
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{catalog_with, expression};
    use super::*;
    use crate::document::base::Summary;
    use crate::document::filter::Filter;
    use crate::vault::catalog::BaseQuery;
    use std::collections::BTreeMap;

    fn source(text: &str) -> String {
        text.to_string()
    }

    #[test]
    fn date_arithmetic_and_summary_forms() {
        pollster::block_on(async {
            let (database, root) =
                catalog_with(&[("A.md", "---\ndone: true\nscore: 7\n---\n")]).await;
            let query = BaseQuery {
                filters: expression("file.mtime > now() - \"1 week\""),
                formulas: BTreeMap::new(),
                projections: vec![source("score"), source("done"), source("missing")],
                sort: vec![],
                group_by: None,
                summaries: vec![
                    (source("score"), SummaryRef::Default(Summary::Sum)),
                    (source("score"), SummaryRef::Default(Summary::Median)),
                    (source("score"), SummaryRef::Default(Summary::Stddev)),
                    (source("done"), SummaryRef::Default(Summary::Checked)),
                    (source("missing"), SummaryRef::Default(Summary::EmptyCount)),
                ],
                limit: None,
                classes: Vec::new(),
            };
            let selection = database.query_base(query).await.unwrap();
            assert_eq!(selection.total_matched, 1, "recent mtime matches");
            assert_eq!(selection.summaries[0].as_i64(), Some(7));
            assert_eq!(selection.summaries[1].as_f64(), Some(7.0));
            // Single row: variance is 0, and SQL-side sqrt(0) = 0.
            assert_eq!(selection.summaries[2].as_f64(), Some(0.0));
            assert_eq!(selection.summaries[3].as_i64(), Some(1));
            assert_eq!(selection.summaries[4].as_i64(), Some(1));
            drop(database);
            let _ = std::fs::remove_dir_all(root);
        });
    }

    #[test]
    fn median_handles_odd_and_even_counts() {
        pollster::block_on(async {
            let median_query = |summaries| BaseQuery {
                filters: Filter::MatchAll,
                formulas: BTreeMap::new(),
                projections: vec![],
                sort: vec![],
                group_by: None,
                summaries,
                classes: Vec::new(),
                limit: None,
            };
            let summary = || vec![(source("score"), SummaryRef::Default(Summary::Median))];

            // Even count: 3, 7, 20, 100 -> average of the middle pair.
            let (database, root) = catalog_with(&[
                ("A.md", "---\nscore: 3\n---\n"),
                ("B.md", "---\nscore: 7\n---\n"),
                ("C.md", "---\nscore: 100\n---\n"),
                ("D.md", "---\nscore: 20\n---\n"),
            ])
            .await;
            let even = database.query_base(median_query(summary())).await.unwrap();
            assert_eq!(
                even.summaries[0].as_f64(),
                Some(13.5),
                "even count averages the middle pair"
            );
            drop(database);
            let _ = std::fs::remove_dir_all(root);

            // Odd count: 3, 7, 100 -> exact middle value.
            let (database, root) = catalog_with(&[
                ("A.md", "---\nscore: 3\n---\n"),
                ("B.md", "---\nscore: 7\n---\n"),
                ("C.md", "---\nscore: 100\n---\n"),
            ])
            .await;
            let odd = database.query_base(median_query(summary())).await.unwrap();
            assert_eq!(
                odd.summaries[0].as_f64(),
                Some(7.0),
                "odd count takes the exact middle value"
            );
            drop(database);
            let _ = std::fs::remove_dir_all(root);
        });
    }
}
