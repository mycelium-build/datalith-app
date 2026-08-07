use std::path::PathBuf;

use gpui::{Point, point};

use conv::{ConvAsUtil, UnwrapOrInf};

use crate::document::graph::{
    BorderStyle as GraphBorderStyle, GraphColor, GraphPhysics, GroupNodeStyle,
    NodeStyle as GraphNodeStyle,
};

pub(super) const INITIAL_LAYOUT_RADIUS: f32 = 256.0;
pub(super) const INITIAL_LAYOUT_REFERENCE_NODES: f32 = 512.0;

// Layout placement is a pseudo-random visual projection;
// the lossy float casts below are deliberate and bounded by the hash's width.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::as_conversions
)]
pub(super) fn deterministic_position(path: &str, node_count: usize) -> Point<f32> {
    // FNV-1a is stable across processes, unlike Rust's randomized default hasher.
    let hash = path
        .as_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
        });
    let angle = (hash as u32) as f32 / u32::MAX as f32 * std::f32::consts::TAU;
    let population_scale = ((node_count as f32).max(INITIAL_LAYOUT_REFERENCE_NODES)
        / INITIAL_LAYOUT_REFERENCE_NODES)
        .sqrt();
    let radial_hash = ((hash >> 32) as u32) as f32 / u32::MAX as f32;
    let radius = (40.0 + radial_hash * (INITIAL_LAYOUT_RADIUS - 40.0)) * population_scale;
    point(angle.cos() * radius, angle.sin() * radius)
}

pub(super) const BASE_NODE_RADIUS: f32 = 4.0;
const LINK_SIZE_LOG_STRENGTH: f32 = 0.8;
const MAX_LINK_SIZE_SCALE: f32 = 4.0;

pub(super) fn incoming_link_scale(incoming: usize) -> f32 {
    let degree: f32 = incoming.approx().unwrap_or_inf();
    degree
        .ln_1p()
        .mul_add(LINK_SIZE_LOG_STRENGTH, 1.0)
        .min(MAX_LINK_SIZE_SCALE)
}

#[derive(Clone, Debug)]
pub(super) struct ViewNode {
    pub(super) relative_path: PathBuf,
    pub(super) label: String,
    pub(super) orphan: bool,
    pub(super) color: Option<GraphColor>,
    pub(super) border_color: Option<GraphColor>,
    pub(super) border_width: f32,
    pub(super) hover_color: Option<GraphColor>,
    pub(super) hover_size: f32,
    pub(super) hover_border_color: Option<GraphColor>,
    pub(super) hover_border_width: f32,
    pub(super) radius: f32,
    pub(super) center_weight: f32,
    pub(super) position: Point<f32>,
    pub(super) velocity: Point<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ViewEdge {
    pub(super) source: usize,
    pub(super) target: usize,
    pub(super) reciprocal: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ViewEdgeStyle {
    pub(super) color: Option<GraphColor>,
    pub(super) width: f32,
}

#[derive(Clone, Debug)]
pub(super) struct GraphSnapshot {
    pub(super) nodes: Vec<ViewNode>,
    pub(super) edges: Vec<ViewEdge>,
    pub(super) edge_color: Option<GraphColor>,
    pub(super) edge_width: f32,
    pub(super) edge_hover_outgoing: ViewEdgeStyle,
    pub(super) edge_hover_incoming: ViewEdgeStyle,
    pub(super) edge_hover_both: ViewEdgeStyle,
    pub(super) arrow: bool,
    pub(super) physics: GraphPhysics,
}

pub(super) struct GraphFocus {
    source: usize,
    included_nodes: Vec<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum IncidentDirection {
    Outgoing,
    Incoming,
    Both,
}

impl GraphFocus {
    pub(super) fn new(snapshot: &GraphSnapshot, source: usize) -> Self {
        let mut included_nodes = vec![false; snapshot.nodes.len()];
        if let Some(included) = included_nodes.get_mut(source) {
            *included = true;
        }
        for edge in &snapshot.edges {
            let sibling = match (edge.source == source, edge.target == source) {
                (true, _) => edge.target,
                (_, true) => edge.source,
                _ => continue,
            };
            if let Some(included) = included_nodes.get_mut(sibling) {
                *included = true;
            }
        }
        Self {
            source,
            included_nodes,
        }
    }

    pub(super) fn includes_node(&self, node: usize) -> bool {
        self.included_nodes.get(node).copied().unwrap_or(false)
    }

    pub(super) const fn direction_of(&self, edge: &ViewEdge) -> Option<IncidentDirection> {
        match (
            edge.reciprocal,
            edge.source == self.source,
            edge.target == self.source,
        ) {
            (true, source_adjacent, target_adjacent) if source_adjacent || target_adjacent => {
                Some(IncidentDirection::Both)
            }
            (false, true, _) => Some(IncidentDirection::Outgoing),
            (false, false, true) => Some(IncidentDirection::Incoming),
            _ => None,
        }
    }
}

pub(super) fn border_width(style: &GraphBorderStyle) -> f32 {
    style
        .width
        .unwrap_or_else(|| if style.color.is_some() { 1.0 } else { 0.0 })
}

pub(super) fn hover_border_width(normal: &GraphBorderStyle, hover: &GraphBorderStyle) -> f32 {
    hover.width.unwrap_or_else(|| {
        if normal.width.is_some() || normal.color.is_some() {
            border_width(normal)
        } else if hover.color.is_some() {
            1.0
        } else {
            0.0
        }
    })
}

pub(super) fn resolve_group_node_style(
    base: &GraphNodeStyle,
    group: Option<&GroupNodeStyle>,
) -> GraphNodeStyle {
    let Some(group) = group else {
        return base.clone();
    };
    let mut resolved = base.clone();
    resolved.color = group.color.or(resolved.color);
    if let Some(size) = group.size {
        resolved.size = Some(resolved.size.unwrap_or(1.0) * size);
    }
    resolved.border.color = group.border.color.or(resolved.border.color);
    resolved.border.width = group.border.width.or(resolved.border.width);
    resolved.hover.color = group.hover.color.or(resolved.hover.color);
    resolved.hover.size = group.hover.size.or(resolved.hover.size);
    resolved.hover.border.color = group.hover.border.color.or(resolved.hover.border.color);
    resolved.hover.border.width = group.hover.border.width.or(resolved.hover.border.width);
    resolved
}

pub(super) fn hit_test_nodes(nodes: &[ViewNode], world: Point<f32>) -> Option<usize> {
    nodes.iter().enumerate().rev().find_map(|(index, node)| {
        let dx = world.x - node.position.x;
        let dy = world.y - node.position.y;
        (dx.mul_add(dx, dy * dy) <= node.radius * node.radius).then_some(index)
    })
}

pub(super) const ALL_LABELS_MIN_ZOOM: f32 = 2.5;
pub(super) const NODE_LABEL_WIDTH: f32 = 240.0;

pub(super) fn label_node_indices(
    nodes: &[ViewNode],
    camera: super::camera::Camera,
    viewport: Point<f32>,
    hovered_node: Option<usize>,
) -> Vec<usize> {
    if camera.zoom < ALL_LABELS_MIN_ZOOM {
        return hovered_node
            .filter(|index| *index < nodes.len())
            .into_iter()
            .collect();
    }

    nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| {
            let center = camera.world_to_screen(node.position, viewport);
            let radius = node.radius
                * if hovered_node == Some(index) {
                    node.hover_size
                } else {
                    1.0
                }
                * camera.zoom;
            (center.x + radius >= 0.0
                && center.x - radius <= viewport.x
                && center.y + radius >= 0.0
                && center.y - radius <= viewport.y)
                .then_some(index)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::indexing_slicing,
        clippy::string_slice
    )]

    use super::*;

    #[test]
    fn node_paths_produce_deterministic_distinct_starting_positions() {
        let node_count = INITIAL_LAYOUT_REFERENCE_NODES.approx_as::<usize>().unwrap();
        let first = deterministic_position("Inbox/day.md", node_count);
        let again = deterministic_position("Inbox/day.md", node_count);
        let other = deterministic_position("Projects/day.md", node_count);

        assert_eq!(first, again);
        assert_ne!(first, other);
        assert!(first.x.is_finite() && first.y.is_finite());
    }

    #[test]
    fn initial_layout_area_grows_with_node_count() {
        let reference_node_count = INITIAL_LAYOUT_REFERENCE_NODES.approx_as::<usize>().unwrap();
        let small = deterministic_position("Inbox/day.md", reference_node_count);
        let large = deterministic_position("Inbox/day.md", reference_node_count * 4);
        let small_radius = small.x.hypot(small.y);
        let large_radius = large.x.hypot(large.y);

        assert!((large_radius / small_radius - 2.0).abs() < 0.001);
    }

    #[test]
    fn node_hit_testing_prefers_the_topmost_node_and_rejects_empty_space() {
        let nodes = vec![
            ViewNode {
                relative_path: "bottom.md".into(),
                label: "bottom".into(),
                orphan: false,
                color: None,
                border_color: None,
                border_width: 0.0,
                hover_color: None,
                hover_size: 1.0,
                hover_border_color: None,
                hover_border_width: 0.0,
                radius: 10.0,
                center_weight: 1.0,
                position: gpui::point(0.0, 0.0),
                velocity: Point::default(),
            },
            ViewNode {
                relative_path: "top.md".into(),
                label: "top".into(),
                orphan: false,
                color: None,
                border_color: None,
                border_width: 0.0,
                hover_color: None,
                hover_size: 1.0,
                hover_border_color: None,
                hover_border_width: 0.0,
                radius: 10.0,
                center_weight: 1.0,
                position: gpui::point(2.0, 0.0),
                velocity: Point::default(),
            },
        ];

        assert_eq!(hit_test_nodes(&nodes, gpui::point(1.0, 0.0)), Some(1));
        assert_eq!(hit_test_nodes(&nodes, gpui::point(30.0, 0.0)), None);
    }

    #[test]
    fn proportional_node_growth_is_logarithmic_and_capped() {
        assert!((incoming_link_scale(0) - 1.0).abs() < 1e-6);
        assert!(incoming_link_scale(10) > incoming_link_scale(1));
        assert!(incoming_link_scale(100) > incoming_link_scale(10));
        assert!((incoming_link_scale(usize::MAX) - 4.0).abs() < 1e-6);
    }
}
