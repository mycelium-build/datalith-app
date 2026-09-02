//! Base definition model: parsed views, formulas, summaries, and labels.

use std::borrow::Cow;
use std::collections::BTreeMap;

use serde::Deserialize;

use crate::document::expr::Expr;
use crate::document::filter::{Filter, PropertyPath};

mod cards;
mod graph;
mod list;
mod parse;
mod table;

// Re-exports give the rest of the crate a stable path; the source modules stay private.
pub use cards::CardsConfig;
pub use graph::{
    BorderStyle, ClassStyle, DirectionalEdgeHoverStyle, GraphClass, GraphColor, GraphConfig,
    GraphDisplay, GraphPhysics, NodeStyle, RawGraphClass,
};
pub use list::ListConfig;
pub use table::TableConfig;

#[derive(Clone, Debug, PartialEq)]
pub struct BaseDefinition {
    pub filters: Filter,
    pub formulas: BTreeMap<String, Expr>,
    pub summaries: BTreeMap<String, CustomSummary>,
    pub properties: BTreeMap<String, PropertyConfig>,
    pub views: Vec<BaseView>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PropertyConfig {
    pub display_name: Option<String>,
}

/// One named rendering configuration inside a Base.
///
/// Shared selection fields (`filters`, `limit`, ordering rules, summaries) apply to every view kind;
/// kind-specific settings live in `kind` and are silently ignored by the other kinds.
#[derive(Clone, Debug, PartialEq)]
pub struct BaseView {
    pub view_type: ViewType,
    pub name: String,
    pub filters: Filter,
    pub limit: Option<usize>,
    pub order: Vec<DisplayProperty>,
    pub sort: Vec<SortRule>,
    pub group_by: Option<GroupRule>,
    pub group_direction: SortDirection,
    pub summaries: BTreeMap<String, String>,
    pub kind: ViewKind,
}

impl BaseView {
    /// The list-view settings when this view renders as a list.
    #[must_use]
    pub const fn as_list(&self) -> Option<&ListConfig> {
        match &self.kind {
            ViewKind::List(config) => Some(config),
            _ => None,
        }
    }

    /// The table-view settings when this view renders as a table.
    #[must_use]
    pub const fn as_table(&self) -> Option<&TableConfig> {
        match &self.kind {
            ViewKind::Table(config) => Some(config),
            _ => None,
        }
    }

    /// The card-view settings when this view renders as cards.
    #[must_use]
    pub const fn as_cards(&self) -> Option<&CardsConfig> {
        match &self.kind {
            ViewKind::Cards(config) => Some(config),
            _ => None,
        }
    }

    /// The graph-view settings when this view renders as a graph.
    #[must_use]
    pub const fn as_graph(&self) -> Option<&GraphConfig> {
        match &self.kind {
            ViewKind::Graph(config) => Some(config),
            _ => None,
        }
    }
}

/// The per-kind settings payload of a view.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum ViewKind {
    List(ListConfig),
    Table(TableConfig),
    Cards(CardsConfig),
    Graph(GraphConfig),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupRule {
    pub property: DisplayProperty,
    pub direction: SortDirection,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayProperty {
    pub source: String,
    pub path: PropertyPath,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SortRule {
    pub source: String,
    pub path: PropertyPath,
    pub direction: SortDirection,
}

/// The default summary functions, each compiled to a documented SQL form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Summary {
    Average,
    Min,
    Max,
    Sum,
    Range,
    Median,
    Stddev,
    Earliest,
    Latest,
    DateRange,
    Checked,
    Unchecked,
    EmptyCount,
    FilledCount,
    Unique,
}

impl Summary {
    /// Resolves a default summary by its facing name.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        match name {
            "Average" => Some(Self::Average),
            "Min" => Some(Self::Min),
            "Max" => Some(Self::Max),
            "Sum" => Some(Self::Sum),
            "Range" => Some(Self::Range),
            "Median" => Some(Self::Median),
            "Stddev" => Some(Self::Stddev),
            "Earliest" => Some(Self::Earliest),
            "Latest" => Some(Self::Latest),
            "Checked" => Some(Self::Checked),
            "Unchecked" => Some(Self::Unchecked),
            "Empty" => Some(Self::EmptyCount),
            "Filled" => Some(Self::FilledCount),
            "Unique" => Some(Self::Unique),
            _ => None,
        }
    }
}

/// A custom summary reduced to one whitelisted aggregate plus post-functions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomSummary {
    pub aggregate: AggregateFn,
    pub post: Vec<PostFn>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AggregateFn {
    Mean,
    Min,
    Max,
    Sum,
    Count,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostFn {
    Round(u32),
    Abs,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ViewType {
    List,
    Table,
    Cards,
    Graph,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CardImageFit {
    #[default]
    Cover,
    Contain,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ListMarkers {
    #[default]
    Bullets,
    Numbers,
    None,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
pub enum TableRowHeight {
    #[serde(rename = "short")]
    Short,
    #[default]
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "tall")]
    Tall,
    #[serde(rename = "extra tall")]
    ExtraTall,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
pub enum SortDirection {
    #[default]
    #[serde(rename = "ASC")]
    Asc,
    #[serde(rename = "DESC")]
    Desc,
}

pub const HARD_RESULT_LIMIT: usize = 50_000;

impl BaseDefinition {
    /// Combines global and view filters with AND for query execution.
    #[must_use]
    pub fn combined_filters(&self, view: &BaseView) -> Filter {
        if matches!(self.filters, Filter::MatchAll) {
            view.filters.clone()
        } else {
            Filter::And(vec![self.filters.clone(), view.filters.clone()])
        }
    }

    /// Resolves a summary name to a default or custom summary reference.
    #[must_use]
    pub fn resolve_summary_ref(&self, name: &str) -> Option<crate::vault::SummaryRef> {
        if let Some(summary) = Summary::named(name) {
            return Some(crate::vault::SummaryRef::Default(summary));
        }
        self.summaries
            .get(name)
            .map(|custom| crate::vault::SummaryRef::Custom(custom.clone()))
    }

    #[must_use]
    pub fn display_label<'a>(&'a self, property: &'a DisplayProperty) -> Cow<'a, str> {
        if let Some(configured) = self
            .properties
            .get(&property.source)
            .and_then(|config| config.display_name.as_deref())
        {
            return Cow::Borrowed(configured);
        }
        Cow::Owned(default_display_label(&property.path))
    }
}

/// Falls back to the property path's last segment when no displayName exists.
#[must_use]
fn default_display_label(path: &PropertyPath) -> String {
    use crate::document::expr::FileField;
    match path {
        PropertyPath::Note(parts) => parts.last().cloned().unwrap_or_default(),
        PropertyPath::File(field) => match field {
            FileField::Name => "name".into(),
            FileField::Ext => "ext".into(),
            FileField::Path => "path".into(),
            FileField::Folder => "folder".into(),
            FileField::Size => "size".into(),
            FileField::Mtime => "modified".into(),
            FileField::Ctime => "created".into(),
            FileField::Links => "links".into(),
            FileField::Tags => "tags".into(),
            FileField::Embeds => "embeds".into(),
            FileField::Backlinks => "backlinks".into(),
            FileField::Properties => "properties".into(),
        },
        PropertyPath::Formula(name) => name.clone(),
    }
}
