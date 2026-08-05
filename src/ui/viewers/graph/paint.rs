use gpui::{
    BorderStyle, Bounds, Corners, Edges, Hsla, PathBuilder, Pixels, Point, Rgba, Window, point, px,
    quad, size,
};
use gpui_component::ActiveTheme;

use crate::document::graph::GraphColor;

use super::camera::Camera;
use super::model::{GraphFocus, GraphSnapshot, IncidentDirection};

pub(super) const HOVER_DIM_OPACITY: f32 = 0.16;

fn graph_color(color: GraphColor) -> Hsla {
    Rgba {
        r: color.red,
        g: color.green,
        b: color.blue,
        a: color.alpha,
    }
    .into()
}

fn screen_point(
    world: Point<f32>,
    camera: Camera,
    viewport: Point<f32>,
    origin: Point<Pixels>,
) -> Point<Pixels> {
    let local = camera.world_to_screen(world, viewport);
    point(origin.x + px(local.x), origin.y + px(local.y))
}

pub(super) fn paint_graph(
    bounds: Bounds<Pixels>,
    snapshot: &GraphSnapshot,
    camera: Camera,
    hovered_node: Option<usize>,
    window: &mut Window,
    cx: &mut gpui::App,
) {
    let viewport = point(f32::from(bounds.size.width), f32::from(bounds.size.height));
    let focus = hovered_node.map(|source| GraphFocus::new(snapshot, source));
    let edge_color = snapshot
        .edge_color
        .map(graph_color)
        .unwrap_or_else(|| cx.theme().border.opacity(0.65));
    let outgoing_hover_color = snapshot
        .edge_hover_outgoing
        .color
        .map(graph_color)
        .unwrap_or(cx.theme().info);
    let incoming_hover_color = snapshot
        .edge_hover_incoming
        .color
        .map(graph_color)
        .unwrap_or(cx.theme().info);
    let both_hover_color = snapshot
        .edge_hover_both
        .color
        .map(graph_color)
        .unwrap_or(cx.theme().info);
    let mut edge_builder = PathBuilder::stroke(px((snapshot.edge_width * camera.zoom).max(0.35)));
    let mut outgoing_edge_builder = PathBuilder::stroke(px((snapshot.edge_hover_outgoing.width
        * camera.zoom)
        .max(0.35)));
    let mut incoming_edge_builder = PathBuilder::stroke(px((snapshot.edge_hover_incoming.width
        * camera.zoom)
        .max(0.35)));
    let mut both_edge_builder =
        PathBuilder::stroke(px((snapshot.edge_hover_both.width * camera.zoom).max(0.35)));
    for edge in &snapshot.edges {
        let source = screen_point(
            snapshot.nodes[edge.source].position,
            camera,
            viewport,
            bounds.origin,
        );
        let target = screen_point(
            snapshot.nodes[edge.target].position,
            camera,
            viewport,
            bounds.origin,
        );
        let builder = match focus.as_ref().and_then(|focus| focus.direction_of(edge)) {
            Some(IncidentDirection::Outgoing) => &mut outgoing_edge_builder,
            Some(IncidentDirection::Incoming) => &mut incoming_edge_builder,
            Some(IncidentDirection::Both) => &mut both_edge_builder,
            None => &mut edge_builder,
        };
        builder.move_to(source);
        builder.line_to(target);
    }
    if let Ok(path) = edge_builder.build() {
        window.paint_path(
            path,
            if focus.is_some() {
                edge_color.opacity(HOVER_DIM_OPACITY)
            } else {
                edge_color
            },
        );
    }
    if focus.is_some() {
        if let Ok(path) = outgoing_edge_builder.build() {
            window.paint_path(path, outgoing_hover_color);
        }
        if let Ok(path) = incoming_edge_builder.build() {
            window.paint_path(path, incoming_hover_color);
        }
        if let Ok(path) = both_edge_builder.build() {
            window.paint_path(path, both_hover_color);
        }
    }

    if snapshot.arrow {
        let mut arrows = PathBuilder::fill();
        let mut outgoing_arrows = PathBuilder::fill();
        let mut incoming_arrows = PathBuilder::fill();
        let mut both_arrows = PathBuilder::fill();
        for edge in &snapshot.edges {
            let source_node = &snapshot.nodes[edge.source];
            let target_node = &snapshot.nodes[edge.target];
            let dx = target_node.position.x - source_node.position.x;
            let dy = target_node.position.y - source_node.position.y;
            let distance = (dx * dx + dy * dy).sqrt().max(0.001);
            let direction = point(dx / distance, dy / distance);
            let target_radius = target_node.radius
                * if hovered_node == Some(edge.target) {
                    target_node.hover_size
                } else {
                    1.0
                };
            let tip_world = point(
                target_node.position.x - direction.x * target_radius,
                target_node.position.y - direction.y * target_radius,
            );
            let tip = screen_point(tip_world, camera, viewport, bounds.origin);
            let arrow_size = px((6.0 * camera.zoom).clamp(3.0, 10.0));
            let perpendicular = point(-direction.y, direction.x);
            let base = point(
                tip.x - px(direction.x * f32::from(arrow_size)),
                tip.y - px(direction.y * f32::from(arrow_size)),
            );
            let arrows = match focus.as_ref().and_then(|focus| focus.direction_of(edge)) {
                Some(IncidentDirection::Outgoing) => &mut outgoing_arrows,
                Some(IncidentDirection::Incoming) => &mut incoming_arrows,
                Some(IncidentDirection::Both) => &mut both_arrows,
                None => &mut arrows,
            };
            arrows.move_to(tip);
            arrows.line_to(point(
                base.x + px(perpendicular.x * f32::from(arrow_size) * 0.55),
                base.y + px(perpendicular.y * f32::from(arrow_size) * 0.55),
            ));
            arrows.line_to(point(
                base.x - px(perpendicular.x * f32::from(arrow_size) * 0.55),
                base.y - px(perpendicular.y * f32::from(arrow_size) * 0.55),
            ));
            arrows.close();
        }
        if let Ok(path) = arrows.build() {
            window.paint_path(
                path,
                if focus.is_some() {
                    edge_color.opacity(HOVER_DIM_OPACITY)
                } else {
                    edge_color
                },
            );
        }
        if focus.is_some() {
            if let Ok(path) = outgoing_arrows.build() {
                window.paint_path(path, outgoing_hover_color);
            }
            if let Ok(path) = incoming_arrows.build() {
                window.paint_path(path, incoming_hover_color);
            }
            if let Ok(path) = both_arrows.build() {
                window.paint_path(path, both_hover_color);
            }
        }
    }

    for (index, node) in snapshot.nodes.iter().enumerate() {
        let hovered = hovered_node == Some(index);
        let center = screen_point(node.position, camera, viewport, bounds.origin);
        let radius =
            px((node.radius * if hovered { node.hover_size } else { 1.0 } * camera.zoom).max(1.0));
        let node_bounds = Bounds::new(
            point(center.x - radius, center.y - radius),
            size(radius * 2.0, radius * 2.0),
        );
        let base_color = node.color.map(graph_color).unwrap_or_else(|| {
            if node.orphan {
                cx.theme().muted_foreground
            } else {
                cx.theme().primary
            }
        });
        let color = if hovered {
            node.hover_color.map(graph_color).unwrap_or(cx.theme().info)
        } else {
            base_color
        };
        let (border_color, border_width) = if hovered {
            (node.hover_border_color, node.hover_border_width)
        } else {
            (node.border_color, node.border_width)
        };
        let mut border_color = border_color.map(graph_color).unwrap_or(color);
        let mut color = color;
        if focus
            .as_ref()
            .is_some_and(|focus| !focus.includes_node(index))
        {
            color = color.opacity(HOVER_DIM_OPACITY);
            border_color = border_color.opacity(HOVER_DIM_OPACITY);
        }
        let border_width = if border_width > 0.0 {
            (border_width * camera.zoom).max(0.35)
        } else {
            0.0
        };
        window.paint_quad(quad(
            node_bounds,
            Corners::all(radius),
            color,
            Edges::all(px(border_width)),
            border_color,
            BorderStyle::default(),
        ));
    }
}
