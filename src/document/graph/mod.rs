mod color;
mod types;
mod validate;

pub use crate::document::filter::Filter;
pub use types::*;
pub use validate::parse_definition;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use yaml_serde::Value;

use crate::document::filter::DocumentFile;
use crate::vault::CatalogFilter;

pub const HARD_NODE_LIMIT: usize = 50_000;
pub const DEFAULT_CENTER_STRENGTH: f32 = 0.002;
pub const DEFAULT_REPULSION_STRENGTH: f32 = 1_024.0;
pub const DEFAULT_LINK_STRENGTH: f32 = 0.04;
pub const DEFAULT_LINK_DISTANCE: f32 = 128.0;

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct GraphDefinition {
    pub(crate) limit: Option<usize>,
    pub(crate) filters: Filter,
    pub(crate) groups: Vec<GraphGroup>,
    pub(crate) display: GraphDisplay,
    pub(crate) physics: GraphPhysics,
}

impl Default for GraphDefinition {
    fn default() -> Self {
        Self {
            limit: None,
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
pub struct GraphGroup {
    pub(crate) name: String,
    pub(crate) filters: Filter,
    pub(crate) node: GroupNodeStyle,
}

#[derive(Clone, Debug)]
pub struct GraphNode {
    pub(crate) path: PathBuf,
    pub(crate) properties: Value,
    pub(crate) links: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GraphEdge {
    pub(crate) source: PathBuf,
    pub(crate) target: PathBuf,
}

pub fn matches_definition(
    definition: &GraphDefinition,
    path: &Path,
    properties: &Value,
    links: &[String],
) -> bool {
    definition.filters.matches(&DocumentFile {
        path,
        properties,
        size_bytes: None,
        modified_ns: None,
        links,
    })
}

pub fn matching_group<'a>(
    definition: &'a GraphDefinition,
    path: &Path,
    properties: &Value,
    links: &[String],
) -> Option<&'a GraphGroup> {
    let file = DocumentFile {
        path,
        properties,
        size_bytes: None,
        modified_ns: None,
        links,
    };
    definition
        .groups
        .iter()
        .find(|group| group.filters.matches(&file))
}

pub struct NodeSelection {
    pub(crate) nodes: Vec<GraphNode>,
    pub(crate) total: usize,
}

pub fn select_nodes(
    definition: &GraphDefinition,
    candidates: impl IntoIterator<Item = GraphNode>,
) -> NodeSelection {
    let mut nodes: Vec<_> = candidates
        .into_iter()
        .filter(|node| matches_definition(definition, &node.path, &node.properties, &node.links))
        .collect();
    let total = nodes.len();
    let effective_limit = definition.limit.unwrap_or(HARD_NODE_LIMIT);
    nodes.truncate(effective_limit);
    NodeSelection { nodes, total }
}

pub fn deduplicate_edges(
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
        let properties = Value::Mapping(yaml_serde::Mapping::default());
        assert!(matches_definition(
            &parse_definition("filters: []").unwrap(),
            Path::new("A.md"),
            &properties,
            &[]
        ));
        assert!(matches_definition(
            &parse_definition("filters:\n  and: []").unwrap(),
            Path::new("A.md"),
            &properties,
            &[]
        ));
        assert!(!matches_definition(
            &parse_definition("filters:\n  or: []").unwrap(),
            Path::new("A.md"),
            &properties,
            &[]
        ));
    }

    #[test]
    fn selects_with_limit_uses_first_group_and_deduplicates_visible_edges() {
        let definition = parse_definition(
            r#"
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
"#,
        )
        .unwrap();
        let properties: Value = yaml_serde::from_str("status: done").unwrap();
        let node = GraphNode {
            path: PathBuf::from("A.md"),
            properties,
            links: Vec::new(),
        };
        let selection = select_nodes(&definition, [node]);
        let nodes = selection.nodes;
        assert_eq!(selection.total, 1);
        assert_eq!(nodes.len(), 1);
        assert_eq!(
            matching_group(
                &definition,
                &nodes[0].path,
                &nodes[0].properties,
                &nodes[0].links
            )
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
