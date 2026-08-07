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
    point(
        px(f32::from(origin.x) + local.x),
        px(f32::from(origin.y) + local.y),
    )
}

struct PaintContext<'a> {
    bounds: Bounds<Pixels>,
    viewport: Point<f32>,
    camera: Camera,
    focus: Option<GraphFocus>,
    edge_color: Hsla,
    outgoing_hover_color: Hsla,
    incoming_hover_color: Hsla,
    both_hover_color: Hsla,
    window: &'a mut Window,
    cx: &'a gpui::App,
}

pub(super) fn paint_graph(
    bounds: Bounds<Pixels>,
    snapshot: &GraphSnapshot,
    camera: Camera,
    hovered_node: Option<usize>,
    window: &mut Window,
    cx: &gpui::App,
) {
    let viewport = point(f32::from(bounds.size.width), f32::from(bounds.size.height));
    let focus = hovered_node.map(|source| GraphFocus::new(snapshot, source));
    let edge_color = snapshot
        .edge_color
        .map_or_else(|| cx.theme().border.opacity(0.65), graph_color);
    let outgoing_hover_color = snapshot
        .edge_hover_outgoing
        .color
        .map_or(cx.theme().info, graph_color);
    let incoming_hover_color = snapshot
        .edge_hover_incoming
        .color
        .map_or(cx.theme().info, graph_color);
    let both_hover_color = snapshot
        .edge_hover_both
        .color
        .map_or(cx.theme().info, graph_color);
    let mut context = PaintContext {
        bounds,
        viewport,
        camera,
        focus,
        edge_color,
        outgoing_hover_color,
        incoming_hover_color,
        both_hover_color,
        window,
        cx,
    };
    paint_edges(&mut context, snapshot);
    if snapshot.arrow {
        paint_arrows(&mut context, snapshot, hovered_node);
    }
    paint_nodes(&mut context, snapshot, hovered_node);
}

fn paint_edges(context: &mut PaintContext, snapshot: &GraphSnapshot) {
    let focus = context.focus.as_ref();
    let mut edge_builder =
        PathBuilder::stroke(px((snapshot.edge_width * context.camera.zoom).max(0.35)));
    let mut outgoing_edge_builder = PathBuilder::stroke(px((snapshot.edge_hover_outgoing.width
        * context.camera.zoom)
        .max(0.35)));
    let mut incoming_edge_builder = PathBuilder::stroke(px((snapshot.edge_hover_incoming.width
        * context.camera.zoom)
        .max(0.35)));
    let mut both_edge_builder = PathBuilder::stroke(px((snapshot.edge_hover_both.width
        * context.camera.zoom)
        .max(0.35)));
    for edge in &snapshot.edges {
        let Some(source) = snapshot.nodes.get(edge.source).map(|node| node.position) else {
            continue;
        };
        let Some(target) = snapshot.nodes.get(edge.target).map(|node| node.position) else {
            continue;
        };
        let source = screen_point(
            source,
            context.camera,
            context.viewport,
            context.bounds.origin,
        );
        let target = screen_point(
            target,
            context.camera,
            context.viewport,
            context.bounds.origin,
        );
        let builder = match focus.and_then(|focus| focus.direction_of(edge)) {
            Some(IncidentDirection::Outgoing) => &mut outgoing_edge_builder,
            Some(IncidentDirection::Incoming) => &mut incoming_edge_builder,
            Some(IncidentDirection::Both) => &mut both_edge_builder,
            None => &mut edge_builder,
        };
        builder.move_to(source);
        builder.line_to(target);
    }
    if let Ok(path) = edge_builder.build() {
        context.window.paint_path(
            path,
            if focus.is_some() {
                context.edge_color.opacity(HOVER_DIM_OPACITY)
            } else {
                context.edge_color
            },
        );
    }
    if focus.is_some() {
        if let Ok(path) = outgoing_edge_builder.build() {
            context
                .window
                .paint_path(path, context.outgoing_hover_color);
        }
        if let Ok(path) = incoming_edge_builder.build() {
            context
                .window
                .paint_path(path, context.incoming_hover_color);
        }
        if let Ok(path) = both_edge_builder.build() {
            context.window.paint_path(path, context.both_hover_color);
        }
    }
}

fn paint_arrows(context: &mut PaintContext, snapshot: &GraphSnapshot, hovered_node: Option<usize>) {
    let focus = context.focus.as_ref();
    let mut arrows = PathBuilder::fill();
    let mut outgoing_arrows = PathBuilder::fill();
    let mut incoming_arrows = PathBuilder::fill();
    let mut both_arrows = PathBuilder::fill();
    for edge in &snapshot.edges {
        let Some(source_node) = snapshot.nodes.get(edge.source) else {
            continue;
        };
        let Some(target_node) = snapshot.nodes.get(edge.target) else {
            continue;
        };
        let dx = target_node.position.x - source_node.position.x;
        let dy = target_node.position.y - source_node.position.y;
        let distance = dx.hypot(dy).max(0.001);
        let direction = point(dx / distance, dy / distance);
        let target_radius = target_node.radius
            * if hovered_node == Some(edge.target) {
                target_node.hover_size
            } else {
                1.0
            };
        let tip_world = point(
            direction.x.mul_add(-target_radius, target_node.position.x),
            direction.y.mul_add(-target_radius, target_node.position.y),
        );
        let tip = screen_point(
            tip_world,
            context.camera,
            context.viewport,
            context.bounds.origin,
        );
        let arrow_size = px((6.0 * context.camera.zoom).clamp(3.0, 10.0));
        let perpendicular = point(-direction.y, direction.x);
        let base = point(
            px(direction
                .x
                .mul_add(-f32::from(arrow_size), f32::from(tip.x))),
            px(direction
                .y
                .mul_add(-f32::from(arrow_size), f32::from(tip.y))),
        );
        let arrows = match focus.and_then(|focus| focus.direction_of(edge)) {
            Some(IncidentDirection::Outgoing) => &mut outgoing_arrows,
            Some(IncidentDirection::Incoming) => &mut incoming_arrows,
            Some(IncidentDirection::Both) => &mut both_arrows,
            None => &mut arrows,
        };
        arrows.move_to(tip);
        let half_wing = f32::from(arrow_size) * 0.55;
        arrows.line_to(point(
            px(perpendicular.x.mul_add(half_wing, f32::from(base.x))),
            px(perpendicular.y.mul_add(half_wing, f32::from(base.y))),
        ));
        arrows.line_to(point(
            px(perpendicular.x.mul_add(-half_wing, f32::from(base.x))),
            px(perpendicular.y.mul_add(-half_wing, f32::from(base.y))),
        ));
        arrows.close();
    }
    if let Ok(path) = arrows.build() {
        context.window.paint_path(
            path,
            if focus.is_some() {
                context.edge_color.opacity(HOVER_DIM_OPACITY)
            } else {
                context.edge_color
            },
        );
    }
    if focus.is_some() {
        if let Ok(path) = outgoing_arrows.build() {
            context
                .window
                .paint_path(path, context.outgoing_hover_color);
        }
        if let Ok(path) = incoming_arrows.build() {
            context
                .window
                .paint_path(path, context.incoming_hover_color);
        }
        if let Ok(path) = both_arrows.build() {
            context.window.paint_path(path, context.both_hover_color);
        }
    }
}

fn paint_nodes(context: &mut PaintContext, snapshot: &GraphSnapshot, hovered_node: Option<usize>) {
    let focus = context.focus.as_ref();
    for (index, node) in snapshot.nodes.iter().enumerate() {
        let hovered = hovered_node == Some(index);
        let center = screen_point(
            node.position,
            context.camera,
            context.viewport,
            context.bounds.origin,
        );
        let radius = px((node.radius
            * if hovered { node.hover_size } else { 1.0 }
            * context.camera.zoom)
            .max(1.0));
        let node_bounds = Bounds::new(
            point(
                px(f32::from(center.x) - f32::from(radius)),
                px(f32::from(center.y) - f32::from(radius)),
            ),
            size(px(f32::from(radius) * 2.0), px(f32::from(radius) * 2.0)),
        );
        let base_color = node.color.map_or_else(
            || {
                if node.orphan {
                    context.cx.theme().muted_foreground
                } else {
                    context.cx.theme().primary
                }
            },
            graph_color,
        );
        let color = if hovered {
            node.hover_color
                .map_or(context.cx.theme().info, graph_color)
        } else {
            base_color
        };
        let (border_color, border_width) = if hovered {
            (node.hover_border_color, node.hover_border_width)
        } else {
            (node.border_color, node.border_width)
        };
        let mut border_color = border_color.map_or(color, graph_color);
        let mut color = color;
        if focus.is_some_and(|focus| !focus.includes_node(index)) {
            color = color.opacity(HOVER_DIM_OPACITY);
            border_color = border_color.opacity(HOVER_DIM_OPACITY);
        }
        let border_width = if border_width > 0.0 {
            (border_width * context.camera.zoom).max(0.35)
        } else {
            0.0
        };
        context.window.paint_quad(quad(
            node_bounds,
            Corners::all(radius),
            color,
            Edges::all(px(border_width)),
            border_color,
            BorderStyle::default(),
        ));
    }
}
