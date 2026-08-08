use std::collections::HashSet;

use anyhow::Result;
use gpui::Point;

use crate::document::graph::{
    DirectionalEdgeHoverStyle, GraphDefinition, GraphEdge, GraphNode, deduplicate_edges,
    matching_group, select_nodes,
};
use crate::vault::{CatalogQuery, VaultCatalog};

use super::model::{
    BASE_NODE_RADIUS, GraphSnapshot, ViewEdge, ViewEdgeStyle, ViewNode, border_width,
    deterministic_position, hover_border_width, incoming_link_scale, resolve_group_node_style,
};

#[derive(Debug)]
pub(super) enum ViewerStatus {
    Loading,
    Ready(GraphSnapshot),
    Empty,
    Error(String),
}

fn view_node(
    definition: &GraphDefinition,
    connected: &HashSet<std::path::PathBuf>,
    incoming: &[usize],
    node_count: usize,
    index: usize,
    node: GraphNode,
) -> ViewNode {
    let orphan = !connected.contains(&node.path);
    let style = if orphan {
        definition.display.orphan.node.clone()
    } else {
        let group = matching_group(definition, &node.path, &node.properties);
        resolve_group_node_style(&definition.display.node, group.map(|group| &group.node))
    };
    let degree_scale = if style.propertional {
        incoming
            .get(index)
            .copied()
            .map_or(1.0, incoming_link_scale)
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
}

fn edge_hover_style(style: &DirectionalEdgeHoverStyle, edge_width: f32) -> ViewEdgeStyle {
    ViewEdgeStyle {
        color: style.color,
        width: style.width.unwrap_or(edge_width),
    }
}

pub(super) fn make_snapshot(
    definition: &GraphDefinition,
    candidates: impl IntoIterator<Item = GraphNode>,
    edges: impl IntoIterator<Item = GraphEdge>,
) -> GraphSnapshot {
    let selection = select_nodes(definition, candidates);
    let rendered = selection.nodes.len();
    let notice = (selection.total > rendered).then(|| {
        format!(
            "Only {rendered} of {} matching nodes rendered; narrow filters or raise the limit.",
            selection.total
        )
    });
    let mut nodes = selection.nodes;
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
        if let Some(count) = incoming.get_mut(edge.target) {
            *count = count.saturating_add(1);
        }
    }

    let node_count = nodes.len();
    let nodes: Vec<_> = nodes
        .into_iter()
        .enumerate()
        .map(|(index, node)| view_node(definition, &connected, &incoming, node_count, index, node))
        .collect();

    let edge_width = definition.display.edge.width.unwrap_or(1.0);
    let hover = &definition.display.edge.hover.direction;
    GraphSnapshot {
        nodes,
        edges: view_edges,
        edge_color: definition.display.edge.color,
        edge_width,
        edge_hover_outgoing: edge_hover_style(&hover.outgoing, edge_width),
        edge_hover_incoming: edge_hover_style(&hover.incoming, edge_width),
        edge_hover_both: edge_hover_style(&hover.both, edge_width),
        arrow: definition.display.edge.arrow,
        physics: definition.physics,
        notice,
    }
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
            limit: None,
        })
        .await?;
    let candidates = selection.documents.into_iter().filter_map(|document| {
        let path = document.path.strip_prefix(&root).ok()?.to_path_buf();
        let properties = document.metadata.map_or_else(
            || yaml_serde::Value::Mapping(yaml_serde::Mapping::default()),
            |metadata| {
                yaml_serde::from_str(&metadata.to_string())
                    .unwrap_or_else(|_| yaml_serde::Value::Mapping(yaml_serde::Mapping::default()))
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
    Ok(make_snapshot(&definition, candidates, edges))
}

#[cfg(test)]
mod tests {
    use super::super::camera::Camera;
    use super::super::model::{
        ALL_LABELS_MIN_ZOOM, BASE_NODE_RADIUS, GraphFocus, IncidentDirection, incoming_link_scale,
        label_node_indices,
    };
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn snapshot_with(
        definition: &GraphDefinition,
        node_paths: &[&str],
        edges: &[(&str, &str)],
    ) -> GraphSnapshot {
        let nodes = node_paths.iter().map(|path| GraphNode {
            path: PathBuf::from(path),
            properties: yaml_serde::Value::Mapping(yaml_serde::Mapping::default()),
        });
        let edges = edges.iter().map(|(source, target)| GraphEdge {
            source: PathBuf::from(*source),
            target: PathBuf::from(*target),
        });
        make_snapshot(definition, nodes, edges)
    }

    #[test]
    fn make_snapshot_renders_all_matching_nodes_when_no_limit_is_set() {
        use crate::document::graph::parse_definition;

        let definition = parse_definition("").unwrap();
        assert_eq!(definition.limit, None);
        let nodes = ["a.md", "b.md", "c.md"].into_iter().map(|path| GraphNode {
            path: PathBuf::from(path),
            properties: yaml_serde::Value::Mapping(yaml_serde::Mapping::default()),
        });
        let snapshot = make_snapshot(&definition, nodes, []);
        assert_eq!(snapshot.nodes.len(), 3);
        assert!(snapshot.notice.is_none());
    }

    #[test]
    fn make_snapshot_truncates_to_the_limit_and_reports_it_in_the_notice() {
        use crate::document::graph::parse_definition;

        let definition = parse_definition("limit: 2").unwrap();
        let nodes = ["a.md", "b.md", "c.md", "d.md"]
            .into_iter()
            .map(|path| GraphNode {
                path: PathBuf::from(path),
                properties: yaml_serde::Value::Mapping(yaml_serde::Mapping::default()),
            });
        let snapshot = make_snapshot(&definition, nodes, []);
        assert_eq!(snapshot.nodes.len(), 2);
        let notice = snapshot.notice.expect("truncation should be reported");
        assert!(notice.contains("Only 2 of 4"), "{notice}");
    }

    #[test]
    fn no_limit_still_caps_at_the_hard_safety_ceiling_with_a_notice() {
        use crate::document::graph::{HARD_NODE_LIMIT, parse_definition};

        let definition = parse_definition("").unwrap();
        assert_eq!(definition.limit, None);
        let nodes = (0..HARD_NODE_LIMIT.saturating_add(1)).map(|index| GraphNode {
            path: PathBuf::from(format!("node-{index}.md")),
            properties: yaml_serde::Value::Mapping(yaml_serde::Mapping::default()),
        });
        let snapshot = make_snapshot(&definition, nodes, []);
        assert_eq!(snapshot.nodes.len(), HARD_NODE_LIMIT);
        let notice = snapshot
            .notice
            .expect("ceiling truncation should be reported");
        assert!(
            notice.contains(&format!("Only {HARD_NODE_LIMIT} of")),
            "{notice}"
        );
    }

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
                properties: yaml_serde::Value::Mapping(yaml_serde::Mapping::default()),
            });
        let mut snapshot = make_snapshot(&definition, nodes, []);
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
            r#"
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
"#,
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

        let snapshot = make_snapshot(&definition, nodes, edges);

        assert_eq!(snapshot.nodes.len(), 2);
        let done = snapshot
            .nodes
            .iter()
            .find(|node| node.relative_path == std::path::Path::new("one.md"))
            .unwrap();
        assert!((done.color.unwrap().red - 1.0).abs() < 1e-6);
        assert!((done.color.unwrap().green - 0.0).abs() < 1e-6);
        let expected_radius = BASE_NODE_RADIUS * 3.0;
        assert!((done.radius - expected_radius).abs() < 0.001);
        assert!((done.border_color.unwrap().red - f32::from(0x44_u8) / 255.0).abs() < 0.001);
        assert!((done.border_width - 1.0).abs() < 0.001);
        assert!((done.hover_color.unwrap().red - f32::from(0x55_u8) / 255.0).abs() < 0.001);
        assert!((done.hover_size - 1.5).abs() < 0.001);
        assert!((done.hover_border_color.unwrap().red - f32::from(0x33_u8) / 255.0).abs() < 0.001);
        assert!((done.hover_border_width - 2.0).abs() < 0.001);
    }

    #[test]
    fn incoming_links_scale_nodes_and_orphan_style_overrides_groups() {
        use crate::document::graph::{GraphEdge, GraphNode, parse_definition};

        let definition = parse_definition(
            r"
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
",
        )
        .unwrap();
        let nodes = ["a.md", "b.md", "c.md", "orphan.md"]
            .into_iter()
            .map(|path| GraphNode {
                path: PathBuf::from(path),
                properties: yaml_serde::Value::Mapping(yaml_serde::Mapping::default()),
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

        let snapshot = make_snapshot(&definition, nodes, edges);
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

        let expected_radius = BASE_NODE_RADIUS * incoming_link_scale(2);
        assert!((linked.radius - expected_radius).abs() < 0.001);
        assert!((linked.color.unwrap().red - 1.0).abs() < 1e-6);
        assert!((orphan.color.unwrap().blue - 1.0).abs() < 1e-6);
        assert!(orphan.orphan);
    }

    #[test]
    fn snapshot_resolves_normal_hover_and_orphan_borders() {
        use crate::document::graph::{GraphEdge, GraphNode, parse_definition};

        let definition = parse_definition(
            r"
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
",
        )
        .unwrap();
        let nodes = ["linked-a.md", "linked-b.md", "orphan.md"]
            .into_iter()
            .map(|path| GraphNode {
                path: path.into(),
                properties: yaml_serde::Value::Mapping(yaml_serde::Mapping::default()),
            });
        let edges = [GraphEdge {
            source: "linked-a.md".into(),
            target: "linked-b.md".into(),
        }];

        let snapshot = make_snapshot(&definition, nodes, edges);
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

        assert!((linked.border_width - 1.0).abs() < 0.001);
        assert!((linked.border_color.unwrap().red - f32::from(0x11_u8) / 255.0).abs() < 0.001);
        assert!((linked.hover_size - 1.5).abs() < 0.001);
        assert!((linked.hover_color.unwrap().red - f32::from(0x44_u8) / 255.0).abs() < 0.001);
        assert!((linked.hover_border_width - 2.0).abs() < 0.001);
        assert_eq!(linked.hover_border_color, linked.border_color);
        assert!((orphan.border_width - 0.5).abs() < 0.001);
        assert!((orphan.hover_border_width - 0.5).abs() < 0.001);
        assert!(
            (orphan.hover_border_color.unwrap().red - f32::from(0x77_u8) / 255.0).abs() < 0.001
        );
    }

    #[test]
    fn hover_focus_includes_both_directions_with_independent_styles() {
        use crate::document::graph::parse_definition;

        let definition = parse_definition(
            r"
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
",
        )
        .unwrap();
        let snapshot = snapshot_with(
            &definition,
            &[
                "source.md",
                "outgoing.md",
                "both.md",
                "inbound.md",
                "next.md",
            ],
            &[
                ("source.md", "outgoing.md"),
                ("source.md", "both.md"),
                ("both.md", "source.md"),
                ("inbound.md", "source.md"),
                ("both.md", "next.md"),
            ],
        );
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
        assert!((snapshot.edge_hover_outgoing.width - 3.0).abs() < 1e-6);
        assert!((snapshot.edge_hover_incoming.width - 4.0).abs() < 1e-6);
        assert!((snapshot.edge_hover_both.width - 5.0).abs() < 1e-6);
        assert!(snapshot.arrow);
        assert!(
            (snapshot.edge_hover_outgoing.color.unwrap().red - f32::from(0xab_u8) / 255.0).abs()
                < 0.001
        );
        assert!(
            (snapshot.edge_hover_incoming.color.unwrap().red - f32::from(0x12_u8) / 255.0).abs()
                < 0.001
        );
        assert!(
            (snapshot.edge_hover_both.color.unwrap().red - f32::from(0xfe_u8) / 255.0).abs()
                < 0.001
        );
    }

    #[test]
    fn edge_hover_widths_inherit_the_base_width() {
        use crate::document::graph::parse_definition;

        let inherited_definition = parse_definition("display:\n  edge:\n    width: 2.25").unwrap();
        let inherited = snapshot_with(&inherited_definition, &[], &[]);

        assert!((inherited.edge_hover_outgoing.width - 2.25).abs() < 1e-6);
        assert!((inherited.edge_hover_incoming.width - 2.25).abs() < 1e-6);
        assert!((inherited.edge_hover_both.width - 2.25).abs() < 1e-6);
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
