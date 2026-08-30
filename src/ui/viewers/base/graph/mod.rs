//! The graph rendering arm of the Base viewer.

mod camera;
pub(super) mod model;
mod paint;
mod physics;
mod snapshot;

use std::path::PathBuf;

use gpui::{
    AnyElement, App, Bounds, Context, FocusHandle, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render,
    ScrollDelta, ScrollWheelEvent, Styled, WeakEntity, Window, div, point, px,
};
use gpui_component::{ActiveTheme, ElementExt, WindowExt, h_flex};

use crate::document::base::GraphConfig;
use crate::document::handler::{FileHandler, FileHandlerEvent};
use crate::ui::notifications;

use self::camera::Camera;
use self::model::{
    GraphFocus, GraphSnapshot, LegendEntry, NODE_LABEL_WIDTH, hit_test_nodes, label_node_indices,
};
use self::physics::Simulation;

use paint::HOVER_DIM_OPACITY;

/// Owns camera, simulation, and pointer state for one embedded graph view.
pub(super) struct GraphState {
    handler: WeakEntity<FileHandler>,
    snapshot: Option<GraphSnapshot>,
    focus_handle: FocusHandle,
    camera: Camera,
    camera_fitted: bool,
    canvas_bounds: Option<Bounds<Pixels>>,
    pointer_position: Option<Point<f32>>,
    hovered_node: Option<usize>,
    interaction: Option<PointerInteraction>,
    simulation: Simulation,
}

fn update_canvas_bounds(entity: &WeakEntity<GraphState>, bounds: Bounds<Pixels>, cx: &mut App) {
    if let Err(error) = entity.update(cx, |state, cx| state.set_canvas_bounds(bounds, cx)) {
        // Not notification because can be spam
        eprintln!("Failed to update Graph View bounds: {error}");
    }
}

#[derive(Clone, Copy, Debug)]
struct PointerInteraction {
    start: Point<f32>,
    last: Point<f32>,
    node: Option<usize>,
    moved: bool,
}

impl GraphState {
    pub(super) fn new(handler: WeakEntity<FileHandler>, cx: &Context<Self>) -> Self {
        Self {
            handler,
            snapshot: None,
            focus_handle: cx.focus_handle(),
            camera: Camera::default(),
            camera_fitted: false,
            canvas_bounds: None,
            pointer_position: None,
            hovered_node: None,
            interaction: None,
            simulation: Simulation::default(),
        }
    }

    /// Swaps in a freshly built snapshot;
    /// the camera only resets when the underlying definition changed (source edit), not on tab switches.
    /// The simulation always restarts so the new layout settles from scratch.
    pub(super) fn set_snapshot(
        &mut self,
        snapshot: Option<GraphSnapshot>,
        reset_view: bool,
        cx: &mut Context<Self>,
    ) {
        if reset_view {
            self.camera = Camera::default();
            self.camera_fitted = false;
        }
        self.pointer_position = None;
        self.hovered_node = None;
        self.interaction = None;
        self.snapshot = snapshot;
        self.simulation = Simulation::default();
        cx.notify();
    }

    fn viewport(&self) -> Option<Point<f32>> {
        self.canvas_bounds
            .map(|bounds| point(f32::from(bounds.size.width), f32::from(bounds.size.height)))
    }

    fn local_position(&self, position: Point<Pixels>) -> Option<Point<f32>> {
        let bounds = self.canvas_bounds?;
        bounds.contains(&position).then(|| {
            point(
                f32::from(position.x) - f32::from(bounds.origin.x),
                f32::from(position.y) - f32::from(bounds.origin.y),
            )
        })
    }

    fn set_canvas_bounds(&mut self, bounds: Bounds<Pixels>, cx: &mut Context<Self>) {
        let changed = self.canvas_bounds != Some(bounds);
        self.canvas_bounds = Some(bounds);
        if !self.camera_fitted
            && bounds.size.width > px(1.0)
            && bounds.size.height > px(1.0)
            && let Some(snapshot) = &self.snapshot
        {
            let viewport = point(f32::from(bounds.size.width), f32::from(bounds.size.height));
            self.camera.fit(&snapshot.nodes, viewport);
            self.camera_fitted = true;
            cx.notify();
        } else if changed {
            cx.notify();
        }
    }

    fn handle_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(local) = self.local_position(event.position) else {
            return;
        };
        let Some(viewport) = self.viewport() else {
            return;
        };
        self.focus_handle.focus(window, cx);
        self.pointer_position = Some(local);
        let world = self.camera.screen_to_world(local, viewport);
        let node = match &mut self.snapshot {
            Some(snapshot) => {
                let node = hit_test_nodes(&snapshot.nodes, world);
                if let Some(index) = node
                    && let Some(node) = snapshot.nodes.get_mut(index)
                {
                    node.velocity = Point::default();
                }
                node
            }
            None => None,
        };
        self.interaction = Some(PointerInteraction {
            start: local,
            last: local,
            node,
            moved: false,
        });
        cx.notify();
    }

    fn handle_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(local) = self.local_position(event.position) else {
            self.pointer_position = None;
            if self.hovered_node.take().is_some() {
                cx.notify();
            }
            return;
        };
        let Some(viewport) = self.viewport() else {
            return;
        };
        self.pointer_position = Some(local);
        if let Some(mut interaction) = self.interaction {
            let drag = point(local.x - interaction.last.x, local.y - interaction.last.y);
            let total_drag = point(local.x - interaction.start.x, local.y - interaction.start.y);
            let was_moved = interaction.moved;
            interaction.moved |= total_drag
                .y
                .mul_add(total_drag.y, total_drag.x * total_drag.x)
                > 16.0;
            if interaction.moved {
                if let Some(index) = interaction.node {
                    let world = self.camera.screen_to_world(local, viewport);
                    if let Some(snapshot) = &mut self.snapshot
                        && let Some(node) = snapshot.nodes.get_mut(index)
                    {
                        node.position = world;
                        node.velocity = Point::default();
                        self.simulation.reheat();
                    }
                } else if was_moved {
                    self.camera.pan.x += drag.x;
                    self.camera.pan.y += drag.y;
                } else {
                    self.camera.pan.x += total_drag.x;
                    self.camera.pan.y += total_drag.y;
                }
            }
            interaction.last = local;
            self.interaction = Some(interaction);
            self.hovered_node = interaction.node;
            cx.notify();
            return;
        }

        let world = self.camera.screen_to_world(local, viewport);
        let hovered = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| hit_test_nodes(&snapshot.nodes, world));
        if hovered != self.hovered_node {
            self.hovered_node = hovered;
            cx.notify();
        }
    }

    fn handle_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(interaction) = self.interaction.take() else {
            return;
        };
        self.pointer_position = self.local_position(event.position);
        let target = if let Some(index) = interaction.node {
            if interaction.moved {
                self.simulation.reheat();
            }
            match &mut self.snapshot {
                Some(snapshot) => snapshot.nodes.get_mut(index).map(|node| {
                    node.velocity = Point::default();
                    node.relative_path.to_string_lossy().replace('\\', "/")
                }),
                None => None,
            }
        } else {
            None
        };
        if !interaction.moved
            && let Some(target) = target
        {
            let new_tab = event.modifiers.platform;
            if let Err(error) = self.handler.update(cx, |_handler, cx| {
                cx.emit(FileHandlerEvent::LinkClicked(target, new_tab));
            }) {
                window.push_notification(notifications::graph_link_open_failed(&error), cx);
            }
        }
        cx.notify();
    }

    fn handle_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(local) = self.local_position(event.position) else {
            return;
        };
        let Some(viewport) = self.viewport() else {
            return;
        };
        self.pointer_position = Some(local);
        let delta = match event.delta {
            ScrollDelta::Pixels(delta) => f32::from(delta.y) * 0.002,
            ScrollDelta::Lines(delta) => delta.y * 0.12,
        };
        self.camera
            .zoom_at(self.camera.zoom * delta.exp(), local, viewport);
        cx.notify();
    }

    pub(super) fn render_canvas(&mut self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let pinned = self.interaction.and_then(|interaction| interaction.node);
        let hover_query = self
            .interaction
            .is_none()
            .then(|| self.pointer_position.zip(self.viewport()))
            .flatten();
        let Some(snapshot) = &mut self.snapshot else {
            return div().into_any_element();
        };
        self.simulation.step(snapshot, pinned);
        if let Some((pointer, viewport)) = hover_query {
            let world = self.camera.screen_to_world(pointer, viewport);
            self.hovered_node = hit_test_nodes(&snapshot.nodes, world);
        }
        if !self.simulation.is_sleeping() {
            window.request_animation_frame();
        }

        let active_hover = self
            .interaction
            .is_none()
            .then_some(self.hovered_node)
            .flatten();
        let snapshot_for_paint = snapshot.clone();
        let camera_for_paint = self.camera;
        let hovered_for_paint = active_hover;
        let entity = cx.entity().downgrade();
        let mut root = div()
            .id("graph-view")
            .size_full()
            .relative()
            .overflow_hidden()
            .bg(cx.theme().background)
            .track_focus(&self.focus_handle)
            .on_mouse_down(MouseButton::Left, cx.listener(Self::handle_mouse_down))
            .on_mouse_move(cx.listener(Self::handle_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::handle_mouse_up))
            .on_scroll_wheel(cx.listener(Self::handle_scroll))
            .on_prepaint(move |bounds, _window, cx| update_canvas_bounds(&entity, bounds, cx))
            .child(
                gpui::canvas(
                    move |bounds, _window, _cx| (bounds, snapshot_for_paint, camera_for_paint),
                    move |_bounds, (bounds, snapshot, camera), window, cx| {
                        paint::paint_graph(
                            bounds,
                            &snapshot,
                            camera,
                            hovered_for_paint,
                            window,
                            cx,
                        );
                    },
                )
                .absolute()
                .size_full(),
            );

        if let Some(bounds) = self.canvas_bounds {
            let viewport = point(f32::from(bounds.size.width), f32::from(bounds.size.height));
            let focus = active_hover.map(|source| GraphFocus::new(snapshot, source));
            for index in label_node_indices(&snapshot.nodes, self.camera, viewport, active_hover) {
                let Some(node) = snapshot.nodes.get(index) else {
                    continue;
                };
                let hovered = active_hover == Some(index);
                let screen = self.camera.world_to_screen(node.position, viewport);
                let radius =
                    (node.radius * if hovered { node.hover_size } else { 1.0 } * self.camera.zoom)
                        .max(1.0);
                let dimmed = focus
                    .as_ref()
                    .is_some_and(|focus| !focus.includes_node(index));
                root = root.child(
                    div()
                        .absolute()
                        .left(px(screen.x - NODE_LABEL_WIDTH / 2.0))
                        .top(px(screen.y + radius + 6.0))
                        .w(px(NODE_LABEL_WIDTH))
                        .text_center()
                        .text_sm()
                        .whitespace_nowrap()
                        .text_color(cx.theme().foreground)
                        .opacity(if dimmed { HOVER_DIM_OPACITY } else { 1.0 })
                        .child(node.label.clone()),
                );
            }
        }

        if let Some(overlay) = render_overlay(&snapshot.legend, &snapshot.summaries, cx) {
            root = root.child(overlay);
        }

        root.into_any_element()
    }
}

/// Top-right column stacking the legend and the summary box.
fn render_overlay(legend: &[LegendEntry], summaries: &[String], cx: &App) -> Option<AnyElement> {
    if legend.is_empty() && summaries.is_empty() {
        return None;
    }
    Some(
        gpui_component::v_flex()
            .absolute()
            .top_2()
            .right_2()
            .items_end()
            .gap_2()
            .children((!legend.is_empty()).then(|| render_legend(legend, cx)))
            .children((!summaries.is_empty()).then(|| render_summary_box(summaries, cx)))
            .into_any_element(),
    )
}

/// One "Pages Sum: 350" line per entry, boxed like the legend.
fn render_summary_box(summaries: &[String], cx: &App) -> AnyElement {
    let lines = summaries.iter().map(|line| {
        div()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(line.clone())
            .into_any_element()
    });
    gpui_component::v_flex()
        .gap_1()
        .p_2()
        .rounded_md()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
        .children(lines)
        .into_any_element()
}

fn render_legend(legend: &[LegendEntry], cx: &App) -> AnyElement {
    let rows = legend.iter().map(|entry| {
        let color = entry.color.map_or(cx.theme().info, paint::graph_color);
        h_flex()
            .items_center()
            .gap_2()
            .child(div().size_2().rounded_full().bg(color))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(entry.name.clone()),
            )
            .into_any_element()
    });
    gpui_component::v_flex()
        .gap_1()
        .p_2()
        .rounded_md()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
        .children(rows)
        .into_any_element()
}

/// Builds a `GraphSnapshot` from query rows for the given view configuration.
///
/// `rows` are `(relative_path, links, class_hits)` triples already stripped of the vault root;
/// class membership comes straight from SQL boolean columns. `summary_lines` feed the summary box.
pub(super) fn build_graph_snapshot(
    config: &GraphConfig,
    root: &std::path::Path,
    rows: impl IntoIterator<Item = (PathBuf, Vec<String>, Vec<bool>)>,
    summary_lines: Vec<String>,
) -> GraphSnapshot {
    snapshot::build(config, root, rows, summary_lines)
}

impl Render for GraphState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.snapshot.is_some() {
            self.render_canvas(window, cx)
        } else {
            div().into_any_element()
        }
    }
}

#[cfg(test)]
pub(in crate::ui) fn test_snapshot(
    base_source: &str,
    rows: Vec<(PathBuf, Vec<String>, Vec<bool>)>,
) -> GraphSnapshot {
    let definition =
        crate::document::base::BaseDefinition::parse(base_source).expect("valid base source");
    let view = definition.views.first().expect("at least one view");
    let config = view.as_graph().expect("graph view").clone();
    snapshot::build(&config, std::path::Path::new(""), rows, Vec::new())
}
