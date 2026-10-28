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
