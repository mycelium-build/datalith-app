//! Snapshot model and query pipeline for the Base viewer.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use crate::document::base::{
    BaseDefinition, DisplayProperty, HARD_RESULT_LIMIT, Summary, ViewType,
};
use crate::document::filter::PropertyPath;
use crate::vault::{BaseDocument, BaseQuery, SummaryRef, VaultCatalog};

use super::cards;
use super::cells::format_scalar_text;

#[derive(Clone, Debug)]
pub(super) struct BaseRow {
    pub(super) path: PathBuf,
    /// Values aligned with `BaseSnapshot::projection_index` sources.
    pub(super) values: Vec<serde_json::Value>,
    pub(super) links: Vec<String>,
    /// Class membership aligned with the view's classes, in order.
    pub(super) class_hits: Vec<bool>,
    pub(super) image: Option<super::cards::CardImage>,
}

/// One entry of the flattened display sequence: a group header or a row.
#[derive(Clone, Debug)]
pub(super) enum BaseItem {
    Header {
        label: String,
        count: usize,
        ordinal: usize,
    },
    Row {
        index: usize,
        ordinal: usize,
    },
}

#[derive(Clone, Debug)]
pub(super) struct BaseSnapshot {
    pub(super) definition: BaseDefinition,
    pub(super) view_index: usize,
    pub(super) rows: Vec<BaseRow>,
    pub(super) items: Vec<BaseItem>,
    pub(super) projection_index: HashMap<String, usize>,
    /// Rendered summary values aligned with the view's `summaries` mapping.
    pub(super) summaries: Vec<SummaryDisplay>,
    pub(super) total: usize,
    pub(super) omitted: usize,
}

#[derive(Clone, Debug)]
pub(super) struct SummaryDisplay {
    pub(super) source: String,
    pub(super) label: String,
    pub(super) title: String,
    pub(super) text: String,
}

pub(super) enum BaseStatus {
    Loading,
    Empty(BaseSnapshot),
    Ready(BaseSnapshot),
    Error(String),
}

impl BaseSnapshot {
    pub(super) fn summary_for(&self, source: &str) -> Option<&SummaryDisplay> {
        self.summaries
            .iter()
            .find(|display| display.source == source)
    }
}
impl BaseSnapshot {
    pub(super) fn projection_value<'a>(
        &'a self,
        row: &'a BaseRow,
        source: &str,
    ) -> Option<&'a serde_json::Value> {
        let index = self.projection_index.get(source)?;
        row.values.get(*index)
    }
}

/// Collects every property source the view needs projected; this list is the
/// single source of truth shared by the query and the row values. Graph views
/// project nothing: labels come from paths and class membership from columns.
pub(super) fn collect_projection_sources(view: &crate::document::base::BaseView) -> Vec<String> {
    if view.view_type == ViewType::Graph {
        return Vec::new();
    }
    let mut sources: Vec<String> = view.order.iter().map(|p| p.source.clone()).collect();
    if let Some(cards) = view.as_cards()
        && let Some(image) = &cards.image
        && !sources.contains(&image.source)
    {
        sources.push(image.source.clone());
    }
    for rule in &view.sort {
        if !sources.contains(&rule.source) {
            sources.push(rule.source.clone());
        }
    }
    if let Some(group) = &view.group_by
        && !sources.contains(&group.property.source)
    {
        sources.push(group.property.source.clone());
    }
    for source in view.summaries.keys() {
        if !sources.contains(source) {
            sources.push(source.clone());
        }
    }
    sources
}

/// Resolves each view summary entry to a default or custom summary reference.
pub(super) fn resolve_view_summaries(
    definition: &BaseDefinition,
    view: &crate::document::base::BaseView,
) -> anyhow::Result<Vec<(String, SummaryRef)>> {
    view.summaries
        .iter()
        .map(|(source, name)| {
            Ok((
                source.clone(),
                resolve_summary_reference(definition, source, name)?,
            ))
        })
        .collect()
}

/// Resolves a view summary entry, selecting the date form of `Range` for
/// date-typed sources (`file.mtime`/`file.ctime`).
fn resolve_summary_reference(
    definition: &BaseDefinition,
    source: &str,
    name: &str,
) -> anyhow::Result<SummaryRef> {
    if name == "Range"
        && matches!(
            crate::document::filter::parse_property(source),
            Ok(PropertyPath::File(
                crate::document::expr::FileField::Mtime | crate::document::expr::FileField::Ctime,
            ))
        )
    {
        return Ok(SummaryRef::Default(Summary::DateRange));
    }
    definition
        .resolve_summary_ref(name)
        .ok_or_else(|| anyhow::anyhow!("unknown summary {name:?}"))
}

/// Flattens rows into the display sequence, inserting a header whenever the
/// group key changes; headers do not consume the row limit.
pub(super) fn build_group_items(
    rows: &[BaseRow],
    projection_index: &HashMap<String, usize>,
    group_source: Option<&String>,
) -> Vec<BaseItem> {
    let Some(group_source) = group_source else {
        return rows
            .iter()
            .enumerate()
            .map(|(index, _)| BaseItem::Row {
                index,
                ordinal: index,
            })
            .collect();
    };
    let group_position = projection_index.get(group_source).copied();
    let mut items = Vec::new();
    let mut current: Option<String> = None;
    let mut header_position: Option<usize> = None;
    let mut header_ordinal = 0usize;
    let mut count = 0usize;
    for (index, row) in rows.iter().enumerate() {
        let label = group_position
            .and_then(|position| row.values.get(position))
            .map_or_else(
                || "Empty".to_string(),
                |value| snapshot_group_key_label(Some(value)),
            );
        if current.as_ref() != Some(&label) {
            current = Some(label.clone());
            items.push(BaseItem::Header {
                label,
                count: 0,
                ordinal: header_ordinal,
            });
            header_ordinal = header_ordinal.saturating_add(1);
            header_position = Some(items.len().saturating_sub(1));
            count = 0;
        }
        items.push(BaseItem::Row {
            index,
            ordinal: count,
        });
        count = count.saturating_add(1);
        if let Some(position) = header_position
            && let Some(BaseItem::Header {
                count: header_count,
                ..
            }) = items.get_mut(position)
        {
            *header_count = count;
        }
    }
    items
}

fn snapshot_group_key_label(value: Option<&serde_json::Value>) -> String {
    match value {
        None | Some(serde_json::Value::Null) => "Empty".to_string(),
        Some(value) => match format_scalar_text(value) {
            text if text.trim().is_empty() => "Empty".to_string(),
            text => text,
        },
    }
}

fn base_row(document: BaseDocument, root: &std::path::Path) -> BaseRow {
    let path = document
        .path
        .strip_prefix(root)
        .map_or_else(|_| document.path.clone(), std::path::Path::to_path_buf);
    let links = document
        .links
        .iter()
        .filter_map(|link| link.strip_prefix(root).ok().map(path_text))
        .collect();
    BaseRow {
        path,
        values: document.values,
        links,
        class_hits: document.class_hits,
        image: None,
    }
}

pub(super) fn path_text(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(super) fn file_name(path: &std::path::Path) -> Option<&str> {
    path.file_stem().and_then(|value| value.to_str())
}
#[allow(clippy::too_many_lines)]
pub(super) async fn load_snapshot(
    definition: BaseDefinition,
    selected_name: Option<&str>,
    catalog: VaultCatalog,
) -> anyhow::Result<BaseSnapshot> {
    let view_index = selected_name
        .and_then(|name| definition.views.iter().position(|view| view.name == name))
        .unwrap_or_default();
    let view = definition
        .views
        .get(view_index)
        .ok_or_else(|| anyhow::anyhow!("Base view is missing"))?;

    let projections = collect_projection_sources(view);
    let summaries = if view.view_type == ViewType::Graph {
        Vec::new()
    } else {
        resolve_view_summaries(&definition, view)?
    };
    let classes = view.as_graph().map_or_else(Vec::new, |config| {
        config
            .classes
            .iter()
            .map(|class| class.filters.clone())
            .collect()
    });
    let selection = catalog
        .query_base(BaseQuery {
            filters: definition.combined_filters(view),
            formulas: definition.formulas.clone(),
            projections: projections.clone(),
            sort: view
                .sort
                .iter()
                .map(|rule| (rule.source.clone(), rule.direction))
                .collect(),
            group_by: view
                .group_by
                .as_ref()
                .map(|group| (group.property.source.clone(), group.direction)),
            summaries,
            classes,
            limit: Some(
                view.limit
                    .unwrap_or(HARD_RESULT_LIMIT)
                    .min(HARD_RESULT_LIMIT),
            ),
        })
        .await?;

    let root = catalog.root();
    let mut rows = selection
        .documents
        .into_iter()
        .map(|document| base_row(document, &root))
        .collect::<Vec<_>>();

    let mut projection_index = HashMap::new();
    for (index, source) in projections.iter().enumerate() {
        projection_index.insert(source.clone(), index);
    }

    if view.view_type == ViewType::Cards
        && let Some(cards_config) = view.as_cards()
        && let Some(image) = &cards_config.image
    {
        let pending = cards::collect_card_image_targets(&image.source, &rows, &projection_index);
        let resolved = if pending.is_empty() {
            BTreeMap::new()
        } else {
            catalog.resolve_paths(pending)
        };
        for row in &mut rows {
            row.image =
                cards::resolve_card_image(&image.source, row, &projection_index, &root, &resolved);
        }
    }

    let group_source = view
        .group_by
        .as_ref()
        .map(|group| group.property.source.clone());
    let items = build_group_items(&rows, &projection_index, group_source.as_ref());

    let summary_labels = view
        .summaries
        .iter()
        .zip(selection.summaries.iter())
        .filter(|(_, value)| !value.is_null())
        .filter_map(|((source, name), value)| {
            let path = crate::document::filter::parse_property(source).ok()?;
            let label = definition
                .display_label(&DisplayProperty {
                    source: source.clone(),
                    path,
                })
                .to_string();
            Some(SummaryDisplay {
                source: source.clone(),
                label,
                title: name.clone(),
                text: format_scalar_text(value),
            })
        })
        .collect::<Vec<_>>();

    let rows_len = rows.len();
    Ok(BaseSnapshot {
        definition,
        view_index,
        projection_index,
        items,
        summaries: summary_labels,
        rows,
        total: selection.total_matched,
        omitted: selection.total_matched.saturating_sub(rows_len),
    })
}
