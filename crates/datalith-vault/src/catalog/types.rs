//! Public catalog types: events, state, and Base query plans.

use std::path::PathBuf;

use datalith_core::base::SortDirection;
pub use datalith_core::base::SummaryRef;
use datalith_core::expr::Expr;
use datalith_core::filter::Filter;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CatalogState {
    Syncing,
    Ready,
    Failed,
}

#[derive(Clone, Debug)]
pub struct CatalogEvent {
    pub paths: Vec<PathBuf>,
    pub structure_changed: bool,
}

/// One catalog match for wiki-link autocomplete.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WikiLinkMatch {
    /// The vault-relative path, including its extension.
    pub path: String,
    /// The name shown in the popup, with `.md` removed for Markdown files.
    pub label: String,
    /// The containing folder, or `None` for files at the vault root.
    pub detail: Option<String>,
}

/// A Base query plan: everything the catalog needs to filter, project, sort, group, and summarize rows for one view.
#[derive(Clone, Debug)]
pub struct BaseQuery {
    /// Global filters combined with the selected view's filters using AND.
    pub filters: Filter,
    pub formulas: std::collections::BTreeMap<String, Expr>,
    /// Property sources projected for every row, in request order.
    pub projections: Vec<String>,
    /// Row ordering; sources must appear in `projections`.
    pub sort: Vec<(String, SortDirection)>,
    pub group_by: Option<(String, SortDirection)>,
    pub summaries: Vec<(String, SummaryRef)>,
    /// Extra boolean filters evaluated per returned row (graph classes);
    /// results land on `BaseDocument::class_hits` in request order.
    pub classes: Vec<Filter>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct BaseDocument {
    pub path: PathBuf,
    /// Projected values aligned with `BaseQuery::projections`.
    pub values: Vec<serde_json::Value>,
    /// Class membership aligned with `BaseQuery::classes`.
    pub class_hits: Vec<bool>,
    pub links: Vec<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct BaseSelection {
    pub documents: Vec<BaseDocument>,
    pub total_matched: usize,
    /// Aggregate values aligned with `BaseQuery::summaries`:
    /// - `None`-keyed entry covers the whole result set (the view footer);
    /// - `Some(key)` entries cover one group each (the group headers).
    pub summaries: Vec<(Option<serde_json::Value>, Vec<serde_json::Value>)>,
}
