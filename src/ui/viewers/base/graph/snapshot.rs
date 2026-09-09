//! Graph snapshot construction from Base query rows.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use gpui_kit::Point;

use crate::document::base::{ClassStyle, DirectionalEdgeHoverStyle, GraphClass, GraphConfig};
use crate::ui::viewers::base::graph::model::{
    BASE_NODE_RADIUS, GraphSnapshot, LegendEntry, ViewEdge, ViewEdgeStyle, ViewNode, border_width,
    deterministic_position, hover_border_width, incoming_link_scale, resolve_class_node_style,
};

/// Builds the renderable snapshot from `(relative_path, links, class_hits)` rows relative to the vault root;
/// the vault `root` turns relative paths into the absolute keys used by edge matching.
#[allow(clippy::too_many_lines)]
pub(super) fn build(
    config: &GraphConfig,
    root: &Path,
    rows: impl IntoIterator<Item = (PathBuf, Vec<String>, Vec<bool>)>,
    summary_lines: Vec<String>,
) -> GraphSnapshot {
    let mut nodes: Vec<GraphCandidate> = Vec::new();
    let mut raw_edges: Vec<(PathBuf, PathBuf)> = Vec::new();
    for (relative, links, class_hits) in rows {
        let source = root.join(&relative);
        for link in &links {
            raw_edges.push((source.clone(), root.join(link)));
        }
        nodes.push(GraphCandidate {
            path: source,
            class_index: class_hits.iter().position(|hit| *hit),
        });
    }

    let selected: HashSet<_> = nodes.iter().map(|node| node.path.clone()).collect();
    let mut edges = deduplicate_edges(raw_edges, &selected);
    let connected: HashSet<_> = edges
        .iter()
        .flat_map(|(source, target)| [source.clone(), target.clone()])
        .collect();

    if !config.display.orphan.show {
        nodes.retain(|node| connected.contains(&node.path));
        let visible: HashSet<_> = nodes.iter().map(|node| node.path.clone()).collect();
        edges.retain(|(source, target)| visible.contains(source) && visible.contains(target));
    }

    let indices: std::collections::HashMap<_, _> = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.path.clone(), index))
        .collect();
    let mut view_edges: Vec<_> = edges
        .iter()
        .filter_map(|(source, target)| {
            Some(ViewEdge {
                source: *indices.get(source)?,
                target: *indices.get(target)?,
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
        .iter()
        .enumerate()
        .map(|(index, node)| view_node(config, &connected, &incoming, node_count, index, node))
        .collect();

    let edge_width = config.display.edge.width.unwrap_or(1.0);
    let hover = &config.display.edge.hover.direction;
    let legend = if config.display.legend {
        config
            .classes
            .iter()
            .map(|class| LegendEntry {
                name: class.name.clone(),
                color: class.node.color.or(config.display.node.color),
            })
            .collect()
    } else {
        Vec::new()
    };
    GraphSnapshot {
        nodes,
        edges: view_edges,
        edge_color: config.display.edge.color,
        edge_width,
        edge_hover_outgoing: edge_hover_style(&hover.outgoing, edge_width),
        edge_hover_incoming: edge_hover_style(&hover.incoming, edge_width),
        edge_hover_both: edge_hover_style(&hover.both, edge_width),
        arrow: config.display.edge.arrow,
        physics: config.physics,
        legend,
        summaries: summary_lines,
    }
}

/// One node candidate returned by the catalog.
struct GraphCandidate {
    /// Absolute path under the vault root.
    path: PathBuf,
    /// Index of the first matching class, or `None` for the default style.
    class_index: Option<usize>,
}

fn class_style(classes: &[GraphClass], index: Option<usize>) -> Option<&ClassStyle> {
    index.and_then(|index| classes.get(index).map(|class| &class.node))
}

fn edge_hover_style(style: &DirectionalEdgeHoverStyle, edge_width: f32) -> ViewEdgeStyle {
    ViewEdgeStyle {
        color: style.color,
        width: style.width.unwrap_or(edge_width),
    }
}

/// Keeps only edges between selected nodes; self-loops and duplicates drop.
fn deduplicate_edges(
    edges: impl IntoIterator<Item = (PathBuf, PathBuf)>,
    selected: &HashSet<PathBuf>,
) -> Vec<(PathBuf, PathBuf)> {
    let mut seen = HashSet::new();
    let mut unique = Vec::new();
    for (source, target) in edges {
        if source == target
            || !selected.contains(&source)
            || !selected.contains(&target)
            || !seen.insert((source.clone(), target.clone()))
        {
            continue;
        }
        unique.push((source, target));
    }
    unique
}

fn view_node(
    config: &GraphConfig,
    connected: &HashSet<PathBuf>,
    incoming: &[usize],
    node_count: usize,
    index: usize,
    node: &GraphCandidate,
) -> ViewNode {
    let orphan = !connected.contains(&node.path);
    let style = if orphan {
        config.display.orphan.node.clone()
    } else {
        let class = class_style(&config.classes, node.class_index);
        resolve_class_node_style(&config.display.node, class)
    };
    let degree_scale = if style.proportional {
        incoming
            .get(index)
            .copied()
            .map_or(1.0, incoming_link_scale)
    } else {
        1.0
    };
    let radius = BASE_NODE_RADIUS * style.size.unwrap_or(1.0) * degree_scale;
    let path_string = node.path.to_string_lossy().replace('\\', "/");
    let label = node
        .path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_string();
    ViewNode {
        label,
        relative_path: node.path.clone(),
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

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::build;
    use crate::document::base::{BaseDefinition, GraphConfig, ViewKind};
    use crate::ui::viewers::base::graph::model::{
        BASE_NODE_RADIUS, GraphFocus, GraphSnapshot, IncidentDirection, incoming_link_scale,
    };

    fn graph_config(base_source: &str) -> GraphConfig {
        let definition = BaseDefinition::parse(base_source).expect("valid base source");
        let ViewKind::Graph(config) = &definition.views[0].kind else {
            panic!("expected a graph view");
        };
        config.clone()
    }

    fn rows_with_links(
        node_paths: &[&str],
        links: &[(&str, Vec<&str>)],
        class_hits: &[bool],
    ) -> Vec<(PathBuf, Vec<String>, Vec<bool>)> {
        node_paths
            .iter()
            .map(|path| {
                let row_links = links
                    .iter()
                    .filter(|(source, _)| *source == *path)
                    .flat_map(|(_, targets)| targets.iter().map(|target| (*target).to_string()))
                    .collect();
                (PathBuf::from(path), row_links, class_hits.to_vec())
            })
            .collect()
    }

    fn make(base_source: &str, node_paths: &[&str], links: &[(&str, Vec<&str>)]) -> GraphSnapshot {
        let config = graph_config(base_source);
        let hits = vec![true; config.classes.len()];
        build(
            &config,
            Path::new(""),
            rows_with_links(node_paths, links, &hits),
            Vec::new(),
        )
    }

    #[test]
    fn summary_lines_fill_the_summary_box() {
        let config = graph_config(
            "views:\n  - type: graph\n    name: G\n    display:\n      legend: false\n",
        );
        let hits = Vec::new();
        let snapshot = build(
            &config,
            Path::new(""),
            rows_with_links(&["a.md"], &[], &hits),
            vec!["Pages Sum: 350".to_string(), "Rating Sum: 8".to_string()],
        );
        assert!(snapshot.legend.is_empty());
        assert_eq!(snapshot.summaries, ["Pages Sum: 350", "Rating Sum: 8"]);
    }

    #[test]
    fn legend_lists_classes_with_resolved_colors_and_respects_the_toggle() {
        let source = r"
views:
  - type: graph
    name: Wiki
    display:
      node:
        color: '#112233'
    classes:
      - name: Explicit
        filters: 'true'
        node:
          color: '#ff0000'
          size: 1.5
      - name: Inherited
        filters: 'true'
        node:
          size: 1.5
";
        let snapshot = make(source, &["a.md"], &[]);
        let names: Vec<_> = snapshot.legend.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["Explicit", "Inherited"]);
        let explicit = &snapshot.legend[0];
        assert!((explicit.color.unwrap().red - 1.0).abs() < 1e-6);
        let inherited = &snapshot.legend[1];
        assert!((inherited.color.unwrap().blue - 0.2).abs() < 1e-6);

        let hidden = make(
            "views:\n  - type: graph\n    name: G\n    display:\n      legend: false\n    classes:\n      - name: A\n        filters: 'true'\n        node:\n          size: 1.5",
            &["a.md"],
            &[],
        );
        assert!(hidden.legend.is_empty());

        let unclassified = make("views:\n  - type: graph\n    name: G", &["a.md"], &[]);
        assert!(unclassified.legend.is_empty());
    }

    #[test]
    fn incoming_links_scale_nodes_and_orphan_style_overrides_classes() {
        let source = r"
views:
  - type: graph
    name: Wiki
    classes:
      - name: Everything
        filters: 'true'
        node:
          color: '#ff0000'
    display:
      orphan:
        show: true
        node:
          color: '#0000ff'
";
        let snapshot = make(
            source,
            &["a.md", "b.md", "c.md", "orphan.md"],
            &[("a.md", vec!["b.md"]), ("c.md", vec!["b.md"])],
        );
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
    fn class_membership_comes_from_the_first_matching_column() {
        let source = r#"
views:
  - type: graph
    name: Wiki
    display:
      node:
        color: '#0000ff'
    classes:
      - name: First
        filters: 'file.inFolder("A")'
        node:
          color: '#ff0000'
      - name: Second
        filters: 'file.inFolder("B")'
        node:
          color: '#00ff00'
"#;
        // SQL decides membership; the layout only applies first-true-wins.
        // one -> two keeps both non-orphan; three stays an orphan.
        let rows = vec![
            (
                PathBuf::from("one.md"),
                vec!["two.md".to_string()],
                vec![true, true],
            ),
            (PathBuf::from("two.md"), vec![], vec![false, true]),
            (PathBuf::from("three.md"), vec![], vec![false, false]),
        ];
        let snapshot = build(&graph_config(source), Path::new(""), rows, Vec::new());

        let first = snapshot
            .nodes
            .iter()
            .find(|node| node.relative_path == std::path::Path::new("one.md"))
            .unwrap();
        let second = snapshot
            .nodes
            .iter()
            .find(|node| node.relative_path == std::path::Path::new("two.md"))
            .unwrap();
        let default = snapshot
            .nodes
            .iter()
            .find(|node| node.relative_path == std::path::Path::new("three.md"))
            .unwrap();
        assert!((first.color.unwrap().red - 1.0).abs() < 1e-6);
        assert!((second.color.unwrap().green - 1.0).abs() < 1e-6);
        assert!(
            default.color.is_none(),
            "orphans keep the plain orphan style"
        );
        assert!(default.orphan);
    }

    #[test]
    fn snapshot_resolves_normal_hover_and_orphan_borders() {
        let source = r"
views:
  - type: graph
    name: Wiki
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
";
        let snapshot = make(
            source,
            &["linked-a.md", "linked-b.md", "orphan.md"],
            &[("linked-a.md", vec!["linked-b.md"])],
        );
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
    fn snapshot_hides_orphans_and_applies_the_first_matching_class() {
        let source = r#"
views:
  - type: graph
    name: Wiki
    classes:
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
        proportional: false
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
"#;
        // SQL marks `one.md` as matching the first class;
        // the second class never applies because the first hit wins.
        let rows = vec![
            ("one.md".into(), Vec::<String>::new(), vec![true, true]),
            (
                "two.md".into(),
                vec!["one.md".to_string()],
                vec![false, false],
            ),
            ("orphan.md".into(), vec![], vec![false, false]),
        ];
        let snapshot = build(
            &graph_config(source),
            std::path::Path::new(""),
            rows,
            Vec::new(),
        );

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
    fn hover_focus_includes_both_directions_with_independent_styles() {
        let source = r"
views:
  - type: graph
    name: Wiki
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
";
        let snapshot = make(
            source,
            &[
                "source.md",
                "outgoing.md",
                "both.md",
                "inbound.md",
                "next.md",
            ],
            &[
                ("source.md", vec!["outgoing.md", "both.md"]),
                ("both.md", vec!["source.md", "next.md"]),
                ("inbound.md", vec!["source.md"]),
            ],
        );
        let index = |path: &str| {
            snapshot
                .nodes
                .iter()
                .position(|node| node.relative_path == std::path::Path::new(path))
                .unwrap()
        };
        let source_index = index("source.md");
        let focus = GraphFocus::new(&snapshot, source_index);

        assert!(focus.includes_node(source_index));
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
        let inherited = make(
            "views:\n  - type: graph\n    name: G\n    display:\n      edge:\n        width: 2.25",
            &[],
            &[],
        );

        assert!((inherited.edge_hover_outgoing.width - 2.25).abs() < 1e-6);
        assert!((inherited.edge_hover_incoming.width - 2.25).abs() < 1e-6);
        assert!((inherited.edge_hover_both.width - 2.25).abs() < 1e-6);
        assert!(!inherited.arrow);
    }
}
