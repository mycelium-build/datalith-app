mod color;
mod filter;
mod types;
mod validate;

pub(crate) use filter::{Expression, Filter};
pub(crate) use types::*;
pub(crate) use validate::parse_definition;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use serde::Deserialize;
use yaml_serde::Value;

use crate::vault::CatalogFilter;

pub(crate) const HARD_NODE_LIMIT: usize = 50_000;
pub(crate) const DEFAULT_CENTER_STRENGTH: f32 = 0.002;
pub(crate) const DEFAULT_REPULSION_STRENGTH: f32 = 1_024.0;
pub(crate) const DEFAULT_LINK_STRENGTH: f32 = 0.04;
pub(crate) const DEFAULT_LINK_DISTANCE: f32 = 128.0;

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct GraphDefinition {
    pub(crate) limit: usize,
    pub(crate) filters: Filter,
    pub(crate) groups: Vec<GraphGroup>,
    pub(crate) display: GraphDisplay,
    pub(crate) physics: GraphPhysics,
}

impl Default for GraphDefinition {
    fn default() -> Self {
        Self {
            limit: HARD_NODE_LIMIT,
            filters: Filter::MatchAll,
            groups: Vec::new(),
            display: GraphDisplay::default(),
            physics: GraphPhysics::default(),
        }
    }
}

impl GraphDefinition {
    pub(crate) fn catalog_filter(&self) -> CatalogFilter {
        self.filters.to_catalog_filter()
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct GraphGroup {
    pub(crate) name: String,
    pub(crate) filters: Filter,
    pub(crate) node: GroupNodeStyle,
}

pub(crate) struct GraphFile<'a> {
    pub(crate) path: &'a Path,
    pub(crate) properties: &'a Value,
}

#[derive(Clone, Debug)]
pub(crate) struct GraphNode {
    pub(crate) path: PathBuf,
    pub(crate) properties: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct GraphEdge {
    pub(crate) source: PathBuf,
    pub(crate) target: PathBuf,
}

pub(crate) fn matches_definition(
    definition: &GraphDefinition,
    path: &Path,
    properties: &Value,
) -> bool {
    let file = GraphFile { path, properties };
    match &definition.filters {
        Filter::Expression(expression)
            if matches!(expression.left, filter::PropertyPath::File(_)) =>
        {
            expression_matches_with_file(expression, &file)
        }
        filter => filter_matches_with_file(filter, &file),
    }
}

fn filter_matches_with_file(filter: &Filter, file: &GraphFile<'_>) -> bool {
    match filter {
        Filter::Expression(expression) => expression_matches_with_file(expression, file),
        Filter::And(filters) => filters
            .iter()
            .all(|filter| filter_matches_with_file(filter, file)),
        Filter::Or(filters) => filters
            .iter()
            .any(|filter| filter_matches_with_file(filter, file)),
        Filter::Not(filter) => !filter_matches_with_file(filter, file),
        Filter::MatchAll => true,
    }
}

fn expression_matches_with_file(expression: &Expression, file: &GraphFile<'_>) -> bool {
    if let filter::Operation::Compare(comparison, expected) = &expression.operation
        && let Some(actual) = filter::file_scalar(&expression.left, file)
    {
        return match comparison {
            filter::Comparison::Equal => actual == *expected,
            filter::Comparison::NotEqual => actual != *expected,
            _ => match (actual, expected) {
                (filter::Scalar::Number(actual), filter::Scalar::Number(expected)) => {
                    match comparison {
                        filter::Comparison::Greater => actual > *expected,
                        filter::Comparison::GreaterEqual => actual >= *expected,
                        filter::Comparison::Less => actual < *expected,
                        filter::Comparison::LessEqual => actual <= *expected,
                        _ => false,
                    }
                }
                _ => false,
            },
        };
    }
    expression.matches(file)
}

pub(crate) fn matching_group<'a>(
    definition: &'a GraphDefinition,
    path: &Path,
    properties: &Value,
) -> Option<&'a GraphGroup> {
    let file = GraphFile { path, properties };
    definition
        .groups
        .iter()
        .find(|group| filter_matches_with_file(&group.filters, &file))
}

pub(crate) fn select_nodes(
    definition: &GraphDefinition,
    candidates: impl IntoIterator<Item = GraphNode>,
) -> Result<Vec<GraphNode>> {
    let nodes: Vec<_> = candidates
        .into_iter()
        .filter(|node| matches_definition(definition, &node.path, &node.properties))
        .collect();
    if nodes.len() > definition.limit {
        bail!(
            "graph matches {} nodes, exceeding limit {}; narrow filters or raise limit",
            nodes.len(),
            definition.limit
        );
    }
    Ok(nodes)
}

pub(crate) fn deduplicate_edges(
    edges: impl IntoIterator<Item = GraphEdge>,
    selected: &HashSet<PathBuf>,
) -> Vec<GraphEdge> {
    let mut unique = HashSet::new();
    edges
        .into_iter()
        .filter(|edge| {
            edge.source != edge.target
                && selected.contains(&edge.source)
                && selected.contains(&edge.target)
                && unique.insert(edge.clone())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_boolean_filters_have_defined_identities() {
        let properties = Value::Mapping(Default::default());
        assert!(matches_definition(
            &parse_definition("filters: []").unwrap(),
            Path::new("A.md"),
            &properties
        ));
        assert!(matches_definition(
            &parse_definition("filters:\n  and: []").unwrap(),
            Path::new("A.md"),
            &properties
        ));
        assert!(!matches_definition(
            &parse_definition("filters:\n  or: []").unwrap(),
            Path::new("A.md"),
            &properties
        ));
    }

    #[test]
    fn selects_with_limit_uses_first_group_and_deduplicates_visible_edges() {
        let definition = parse_definition(
            r##"
limit: 1
groups:
  - name: First
    filters: 'status == "done"'
    node:
      color: '#ff0000'
  - name: Second
    filters: 'status == "done"'
    node:
      color: '#00ff00'
"##,
        )
        .unwrap();
        let properties: Value = yaml_serde::from_str("status: done").unwrap();
        let node = GraphNode {
            path: PathBuf::from("A.md"),
            properties,
        };
        let nodes = select_nodes(&definition, [node]).unwrap();
        assert_eq!(
            matching_group(&definition, &nodes[0].path, &nodes[0].properties)
                .unwrap()
                .name,
            "First"
        );

        let selected = HashSet::from([PathBuf::from("A.md"), PathBuf::from("B.md")]);
        let edge = GraphEdge {
            source: PathBuf::from("A.md"),
            target: PathBuf::from("B.md"),
        };
        let self_edge = GraphEdge {
            source: PathBuf::from("A.md"),
            target: PathBuf::from("A.md"),
        };
        assert_eq!(
            deduplicate_edges([edge.clone(), edge, self_edge], &selected).len(),
            1
        );
    }
}
