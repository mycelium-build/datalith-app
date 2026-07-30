use std::collections::HashSet;

use anyhow::{Result, bail};
use gpui::Point;

use crate::document::graph::{
    GraphDefinition, GraphEdge, GraphNode, deduplicate_edges, matching_group, select_nodes,
};
use crate::vault::{CatalogQuery, VaultCatalog};

use super::model::{
    BASE_NODE_RADIUS, GraphSnapshot, ViewEdge, ViewEdgeStyle, ViewNode, border_width,
    deterministic_position, hover_border_width, incoming_link_scale,
    resolve_group_node_style,
};

#[derive(Debug)]
pub(super) enum ViewerStatus {
    Loading,
    Ready(GraphSnapshot),
    Empty,
    Error(String),
}

pub(super) fn make_snapshot(
    definition: &GraphDefinition,
    candidates: impl IntoIterator<Item = GraphNode>,
    edges: impl IntoIterator<Item = GraphEdge>,
) -> Result<GraphSnapshot> {
    let mut nodes = select_nodes(definition, candidates)?;
    let selected: HashSet<_> = nodes.iter().map(|node| node.path.clone()).collect();
    let mut edges = deduplicate_edges(edges, &selected);
    let connected: HashSet<_> = edges
        .iter()
        .flat_map(|edge| [&edge.source, &edge.target])
        .cloned()
        .collect();

    if !definition.display.orphan.show {
        nodes.retain(|node| connected.contains(&node.path));
        let visible: HashSet<_> = nodes.iter().map(|node| node.path.clone()).collect();
        edges.retain(|edge| visible.contains(&edge.source) && visible.contains(&edge.target));
    }

    let indices: std::collections::HashMap<_, _> = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.path.clone(), index))
        .collect();
    let mut view_edges: Vec<_> = edges
        .iter()
        .filter_map(|edge| {
            Some(ViewEdge {
                source: *indices.get(&edge.source)?,
                target: *indices.get(&edge.target)?,
                reciprocal: false,
            })
        })
        .collect();
    let directed_edges: HashSet<_> = view_edges
        .iter()
        .map(|edge| (edge.source, edge.target))
        .collect();
    for edge in &mut view_edges {
        edge.reciprocal = directed_edges.contains(&(edge.target, edge.source));
    }
    let mut incoming = vec![0_usize; nodes.len()];
    for edge in &view_edges {
        incoming[edge.target] += 1;
    }

    let node_count = nodes.len();
    let nodes = nodes
        .into_iter()
        .enumerate()
        .map(|(index, node)| {
            let orphan = !connected.contains(&node.path);
            let style = if orphan {
                definition.display.orphan.node.clone()
            } else {
                let group = matching_group(definition, &node.path, &node.properties);
                resolve_group_node_style(&definition.display.node, group.map(|group| &group.node))
            };
            let degree_scale = if style.propertional {
                incoming_link_scale(incoming[index])
            } else {
                1.0
            };
            let radius = BASE_NODE_RADIUS * style.size.unwrap_or(1.0) * degree_scale;
            let path_string = node.path.to_string_lossy().replace('\\', "/");
            ViewNode {
                label: node
                    .path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or_default()
                    .to_string(),
                relative_path: node.path,
                orphan,
                color: style.color,
                border_color: style.border.color,
                border_width: border_width(&style.border),
                hover_color: style.hover.color,
                hover_size: style.hover.size.unwrap_or(1.0),
                hover_border_color: style.hover.border.color.or(style.border.color),
                hover_border_width: hover_border_width(&style.border, &style.hover.border),
                radius,
                center_weight: radius / BASE_NODE_RADIUS,
                position: deterministic_position(&path_string, node_count),
                velocity: Point::default(),
            }
        })
        .collect();

    let edge_width = definition.display.edge.width.unwrap_or(1.0);
    Ok(GraphSnapshot {
        nodes,
        edges: view_edges,
        edge_color: definition.display.edge.color,
        edge_width,
        edge_hover_outgoing: ViewEdgeStyle {
            color: definition.display.edge.hover.direction.outgoing.color,
            width: definition
                .display
                .edge
                .hover
                .direction
                .outgoing
                .width
                .unwrap_or(edge_width),
        },
        edge_hover_incoming: ViewEdgeStyle {
            color: definition.display.edge.hover.direction.incoming.color,
            width: definition
                .display
                .edge
                .hover
                .direction
                .incoming
                .width
                .unwrap_or(edge_width),
        },
        edge_hover_both: ViewEdgeStyle {
            color: definition.display.edge.hover.direction.both.color,
            width: definition
                .display
                .edge
                .hover
                .direction
                .both
                .width
                .unwrap_or(edge_width),
        },
        arrow: definition.display.edge.arrow,
        physics: definition.physics,
    })
}

pub(super) async fn load_snapshot(
    definition: GraphDefinition,
    catalog: VaultCatalog,
) -> Result<GraphSnapshot> {
    let root = catalog.root();
    let selection = catalog
        .query_documents_with_links(CatalogQuery {
            extension: Some("md".into()),
            filter: definition.catalog_filter(),
            limit: definition.limit,
        })
        .await?;
    if selection.exceeded_limit {
        bail!(
            "graph matches more than {} nodes; narrow filters or raise limit",
            definition.limit
        );
    }
    let candidates = selection.documents.into_iter().filter_map(|document| {
        let path = document.path.strip_prefix(&root).ok()?.to_path_buf();
        let properties = document.metadata.map_or_else(
            || yaml_serde::Value::Mapping(Default::default()),
            |metadata| {
                yaml_serde::from_str(&metadata.to_string())
                    .unwrap_or_else(|_| yaml_serde::Value::Mapping(Default::default()))
            },
        );
        Some(GraphNode { path, properties })
    });
    let edges = selection.links.into_iter().filter_map(|edge| {
        Some(GraphEdge {
            source: edge.source.strip_prefix(&root).ok()?.to_path_buf(),
            target: edge.target.strip_prefix(&root).ok()?.to_path_buf(),
        })
    });
    make_snapshot(&definition, candidates, edges)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::model::{
        BASE_NODE_RADIUS, ALL_LABELS_MIN_ZOOM, GraphFocus, IncidentDirection,
        label_node_indices, incoming_link_scale,
    };
    use super::super::camera::Camera;
    use std::fs;
    use std::path::PathBuf;

    fn graph_catalog(root: &std::path::Path) -> VaultCatalog {
        use crate::document::file_types::{FileTypeCapabilities, RegisteredFileTypes};

        let types = RegisteredFileTypes::new([
            (
                "md".to_string(),
                FileTypeCapabilities {
                    text_search: true,
                    wiki_links: true,
                    yaml_frontmatter: true,
                },
            ),
            (
                "graph".to_string(),
                FileTypeCapabilities {
                    text_search: false,
                    wiki_links: false,
                    yaml_frontmatter: false,
                },
            ),
            (
                "todotxt".to_string(),
                FileTypeCapabilities {
                    text_search: true,
                    wiki_links: false,
                    yaml_frontmatter: false,
                },
            ),
        ]);
        let catalog = VaultCatalog::open(root.to_path_buf(), types).unwrap();
        catalog.wait_until_ready(std::time::Duration::from_secs(5));
        catalog
    }

    fn graph_test_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("datalith-graph-{name}-{}", std::process::id()))
    }

    #[test]
    fn high_zoom_labels_only_nodes_visible_in_the_viewport() {
        use crate::document::graph::{GraphNode, parse_definition};

        let definition = parse_definition("").unwrap();
        let nodes = ["visible.md", "offscreen.md"]
            .into_iter()
            .map(|path| GraphNode {
                path: path.into(),
                properties: yaml_serde::Value::Mapping(Default::default()),
            });
        let mut snapshot = make_snapshot(&definition, nodes, []).unwrap();
        snapshot.nodes[0].position = gpui::point(0.0, 0.0);
        snapshot.nodes[1].position = gpui::point(1_000.0, 0.0);
        let viewport = gpui::point(800.0, 600.0);
        let high_zoom = Camera {
            zoom: ALL_LABELS_MIN_ZOOM,
            ..Camera::default()
        };

        assert_eq!(
            label_node_indices(&snapshot.nodes, high_zoom, viewport, None),
            [0]
        );
        assert_eq!(
            label_node_indices(&snapshot.nodes, Camera::default(), viewport, Some(0)),
            [0]
        );
        assert!(label_node_indices(&snapshot.nodes, Camera::default(), viewport, None).is_empty());
    }

    #[test]
    fn snapshot_hides_orphans_and_applies_the_first_matching_group() {
        use crate::document::graph::{GraphEdge, GraphNode, parse_definition};

        let definition = parse_definition(
            r##"
groups:
  - name: Done
    filters: 'status == "done"'
    node:
      color: '#ff0000'
      size: 1.5
      border:
        color: '#444444'
      hover:
        color: '#555555'
        size: 1.5
        border:
          width: 2.0
  - name: Later
    filters: 'status == "done"'
    node:
      color: '#00ff00'
display:
  node:
    color: '#0000ff'
    size: 2.0
    propertional: false
    border:
      color: '#111111'
      width: 1.0
    hover:
      color: '#222222'
      size: 1.25
      border:
        color: '#333333'
        width: 1.0
  orphan:
    show: false
"##,
        )
        .unwrap();
        let nodes = [
            ("one.md", "status: done"),
            ("two.md", "status: open"),
            ("orphan.md", "status: open"),
        ]
        .into_iter()
        .map(|(path, properties)| GraphNode {
            path: PathBuf::from(path),
            properties: yaml_serde::from_str(properties).unwrap(),
        });
        let edges = [GraphEdge {
            source: PathBuf::from("two.md"),
            target: PathBuf::from("one.md"),
        }];

        let snapshot = make_snapshot(&definition, nodes, edges).unwrap();

        assert_eq!(snapshot.nodes.len(), 2);
        let done = snapshot
            .nodes
            .iter()
            .find(|node| node.relative_path == std::path::Path::new("one.md"))
            .unwrap();
        assert_eq!(done.color.unwrap().red, 1.0);
        assert_eq!(done.color.unwrap().green, 0.0);
        assert!((done.radius - BASE_NODE_RADIUS * 3.0).abs() < 0.001);
        assert_eq!(done.border_color.unwrap().red, 0x44 as f32 / 255.0);
        assert_eq!(done.border_width, 1.0);
        assert_eq!(done.hover_color.unwrap().red, 0x55 as f32 / 255.0);
        assert_eq!(done.hover_size, 1.5);
        assert_eq!(done.hover_border_color.unwrap().red, 0x33 as f32 / 255.0);
        assert_eq!(done.hover_border_width, 2.0);
    }

    #[test]
    fn incoming_links_scale_nodes_and_orphan_style_overrides_groups() {
        use crate::document::graph::{GraphEdge, GraphNode, parse_definition};

        let definition = parse_definition(
            r##"
groups:
  - name: Everything
    filters: []
    node:
      color: '#ff0000'
display:
  orphan:
    show: true
    node:
      color: '#0000ff'
"##,
        )
        .unwrap();
        let nodes = ["a.md", "b.md", "c.md", "orphan.md"]
            .into_iter()
            .map(|path| GraphNode {
                path: PathBuf::from(path),
                properties: yaml_serde::Value::Mapping(Default::default()),
            });
        let edges = [
            GraphEdge {
                source: "a.md".into(),
                target: "b.md".into(),
            },
            GraphEdge {
                source: "c.md".into(),
                target: "b.md".into(),
            },
        ];

        let snapshot = make_snapshot(&definition, nodes, edges).unwrap();
        let linked = snapshot
            .nodes
            .iter()
            .find(|node| node.relative_path == std::path::Path::new("b.md"))
            .unwrap();
        let orphan = snapshot
            .nodes
            .iter()
            .find(|node| node.relative_path == std::path::Path::new("orphan.md"))
            .unwrap();

        assert!((linked.radius - BASE_NODE_RADIUS * incoming_link_scale(2)).abs() < 0.001);
        assert_eq!(linked.color.unwrap().red, 1.0);
        assert_eq!(orphan.color.unwrap().blue, 1.0);
        assert!(orphan.orphan);
    }

    #[test]
    fn snapshot_resolves_normal_hover_and_orphan_borders() {
        use crate::document::graph::{GraphEdge, GraphNode, parse_definition};

        let definition = parse_definition(
            r##"
display:
  node:
    border:
      color: '#112233'
    hover:
      color: '#445566'
      size: 1.5
      border:
        width: 2.0
  orphan:
    node:
      border:
        width: 0.5
      hover:
        border:
          color: '#778899'
"##,
        )
        .unwrap();
        let nodes = ["linked-a.md", "linked-b.md", "orphan.md"]
            .into_iter()
            .map(|path| GraphNode {
                path: path.into(),
                properties: yaml_serde::Value::Mapping(Default::default()),
            });
        let edges = [GraphEdge {
            source: "linked-a.md".into(),
            target: "linked-b.md".into(),
        }];

        let snapshot = make_snapshot(&definition, nodes, edges).unwrap();
        let linked = snapshot
            .nodes
            .iter()
            .find(|node| node.relative_path == std::path::Path::new("linked-a.md"))
            .unwrap();
        let orphan = snapshot
            .nodes
            .iter()
            .find(|node| node.relative_path == std::path::Path::new("orphan.md"))
            .unwrap();

        assert_eq!(linked.border_width, 1.0);
        assert_eq!(linked.border_color.unwrap().red, 0x11 as f32 / 255.0);
        assert_eq!(linked.hover_size, 1.5);
        assert_eq!(linked.hover_color.unwrap().red, 0x44 as f32 / 255.0);
        assert_eq!(linked.hover_border_width, 2.0);
        assert_eq!(linked.hover_border_color, linked.border_color);
        assert_eq!(orphan.border_width, 0.5);
        assert_eq!(orphan.hover_border_width, 0.5);
        assert_eq!(orphan.hover_border_color.unwrap().red, 0x77 as f32 / 255.0);
    }

    #[test]
    fn hover_focus_includes_both_directions_with_independent_styles() {
        use crate::document::graph::{GraphEdge, GraphNode, parse_definition};

        let definition = parse_definition(
            r##"
display:
  edge:
    width: 1.5
    arrow: true
    hover:
      direction:
        outgoing:
          color: '#abcdef'
          width: 3.0
        incoming:
          color: '#123456'
          width: 4.0
        both:
          color: '#fedcba'
          width: 5.0
"##,
        )
        .unwrap();
        let nodes = [
            "source.md",
            "outgoing.md",
            "both.md",
            "inbound.md",
            "next.md",
        ]
        .into_iter()
        .map(|path| GraphNode {
            path: path.into(),
            properties: yaml_serde::Value::Mapping(Default::default()),
        });
        let edges = [
            GraphEdge {
                source: "source.md".into(),
                target: "outgoing.md".into(),
            },
            GraphEdge {
                source: "source.md".into(),
                target: "both.md".into(),
            },
            GraphEdge {
                source: "both.md".into(),
                target: "source.md".into(),
            },
            GraphEdge {
                source: "inbound.md".into(),
                target: "source.md".into(),
            },
            GraphEdge {
                source: "both.md".into(),
                target: "next.md".into(),
            },
        ];
        let snapshot = make_snapshot(&definition, nodes, edges).unwrap();
        let index = |path: &str| {
            snapshot
                .nodes
                .iter()
                .position(|node| node.relative_path == std::path::Path::new(path))
                .unwrap()
        };
        let source = index("source.md");
        let focus = GraphFocus::new(&snapshot, source);

        assert!(focus.includes_node(source));
        assert!(focus.includes_node(index("outgoing.md")));
        assert!(focus.includes_node(index("both.md")));
        assert!(focus.includes_node(index("inbound.md")));
        assert!(!focus.includes_node(index("next.md")));
        let direction = |from: &str, to: &str| {
            let edge = snapshot
                .edges
                .iter()
                .find(|edge| edge.source == index(from) && edge.target == index(to))
                .unwrap();
            focus.direction_of(edge)
        };
        assert_eq!(
            direction("source.md", "outgoing.md"),
            Some(IncidentDirection::Outgoing)
        );
        assert_eq!(
            direction("inbound.md", "source.md"),
            Some(IncidentDirection::Incoming)
        );
        assert_eq!(
            direction("source.md", "both.md"),
            Some(IncidentDirection::Both)
        );
        assert_eq!(
            direction("both.md", "source.md"),
            Some(IncidentDirection::Both)
        );
        assert_eq!(snapshot.edge_hover_outgoing.width, 3.0);
        assert_eq!(snapshot.edge_hover_incoming.width, 4.0);
        assert_eq!(snapshot.edge_hover_both.width, 5.0);
        assert!(snapshot.arrow);
        assert_eq!(
            snapshot.edge_hover_outgoing.color.unwrap().red,
            0xab as f32 / 255.0
        );
        assert_eq!(
            snapshot.edge_hover_incoming.color.unwrap().red,
            0x12 as f32 / 255.0
        );
        assert_eq!(
            snapshot.edge_hover_both.color.unwrap().red,
            0xfe as f32 / 255.0
        );

        let inherited_definition = parse_definition("display:\n  edge:\n    width: 2.25").unwrap();
        let inherited = make_snapshot(
            &inherited_definition,
            std::iter::empty::<GraphNode>(),
            std::iter::empty::<GraphEdge>(),
        )
        .unwrap();
        assert_eq!(inherited.edge_hover_outgoing.width, 2.25);
        assert_eq!(inherited.edge_hover_incoming.width, 2.25);
        assert_eq!(inherited.edge_hover_both.width, 2.25);
        assert!(!inherited.arrow);
    }

    #[test]
    fn catalog_snapshot_selects_only_markdown_and_tolerates_invalid_frontmatter() {
        let root = graph_test_root("markdown-only");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("selected.md"), "---\nstatus: done\n---\nSelected").unwrap();
        fs::write(
            root.join("invalid.md"),
            "---\nd:\n 1. a\n 1. b\nloose text\n---\nInvalid",
        )
        .unwrap();
        fs::write(root.join("not-a-note.todotxt"), "status: done").unwrap();
        fs::write(root.join("view.graph"), "filters: []").unwrap();
        let catalog = graph_catalog(&root);
        let definition =
            crate::document::graph::parse_definition("filters: 'status == \"done\"'").unwrap();

        let snapshot = pollster::block_on(load_snapshot(definition, catalog.clone())).unwrap();

        assert_eq!(snapshot.nodes.len(), 1);
        assert_eq!(
            snapshot.nodes[0].relative_path,
            PathBuf::from("selected.md")
        );
        drop(catalog);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn catalog_snapshot_omits_a_new_unreadable_markdown_file() {
        let root = graph_test_root("unreadable");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("binary.md"), [0xff, 0xfe, 0xfd]).unwrap();
        let catalog = graph_catalog(&root);
        let definition = crate::document::graph::parse_definition("").unwrap();

        let snapshot = pollster::block_on(load_snapshot(definition, catalog.clone())).unwrap();

        assert!(snapshot.nodes.is_empty());
        drop(catalog);
        let _ = fs::remove_dir_all(root);
    }
}
