//! Public catalog types: events, state, and Base query plans.

use std::path::PathBuf;

use crate::document::base::{CustomSummary, SortDirection, Summary};
use crate::document::expr::Expr;
use crate::document::filter::Filter;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CatalogState {
    Syncing,
    Ready,
    Failed,
}

#[derive(Clone, Debug)]
pub struct CatalogEvent {
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) structure_changed: bool,
}

/// A Base query plan: everything the catalog needs to filter, project, sort, group, and summarize rows for one view.
#[derive(Clone, Debug)]
pub struct BaseQuery {
    /// Global filters combined with the selected view's filters using AND.
    pub(crate) filters: Filter,
    pub(crate) formulas: std::collections::BTreeMap<String, Expr>,
    /// Property sources projected for every row, in request order.
    pub(crate) projections: Vec<String>,
    /// Row ordering; sources must appear in `projections`.
    pub(crate) sort: Vec<(String, SortDirection)>,
    pub(crate) group_by: Option<(String, SortDirection)>,
    pub(crate) summaries: Vec<(String, SummaryRef)>,
    /// Extra boolean filters evaluated per returned row (graph classes);
    /// results land on `BaseDocument::class_hits` in request order.
    pub(crate) classes: Vec<Filter>,
    pub(crate) limit: Option<usize>,
}

#[derive(Clone, Debug)]
pub enum SummaryRef {
    Default(Summary),
    Custom(CustomSummary),
}

#[derive(Clone, Debug)]
pub struct BaseDocument {
    pub(crate) path: PathBuf,
    #[allow(dead_code)]
    pub(crate) metadata: Option<serde_json::Value>,
    #[allow(dead_code)]
    pub(crate) created_ns: i64,
    /// Projected values aligned with `BaseQuery::projections`.
    pub(crate) values: Vec<serde_json::Value>,
    /// Class membership aligned with `BaseQuery::classes`.
    pub(crate) class_hits: Vec<bool>,
    pub(crate) links: Vec<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct BaseSelection {
    pub(crate) documents: Vec<BaseDocument>,
    pub(crate) total_matched: usize,
    /// Aggregate values aligned with `BaseQuery::summaries`.
    pub(crate) summaries: Vec<serde_json::Value>,
}
