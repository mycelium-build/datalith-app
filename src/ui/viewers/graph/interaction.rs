use gpui::{
    AnyElement, App, AppContext, Bounds, Context, Entity, FocusHandle, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels,
    Point, ScrollDelta, ScrollWheelEvent, Styled, Task, WeakEntity, Window, div, point, px,
};
use gpui_component::input::InputState;
use gpui_component::{ActiveTheme, ElementExt};

use crate::document::handler::{FileHandler, FileHandlerEvent};
use crate::vault::VaultCatalog;

use super::camera::Camera;
use super::model::{GraphFocus, NODE_LABEL_WIDTH, hit_test_nodes, label_node_indices};
use super::physics::Simulation;
use super::snapshot::{ViewerStatus, load_snapshot};

use super::paint::HOVER_DIM_OPACITY;

#[derive(Clone, Copy, Debug)]
struct PointerInteraction {
    start: Point<f32>,
    last: Point<f32>,
    node: Option<usize>,
    moved: bool,
}

pub(super) struct GraphViewState {
    input: Entity<InputState>,
    pub(super) catalog: Option<VaultCatalog>,
    handler: WeakEntity<FileHandler>,
    pub(super) status: ViewerStatus,
    pub(super) focus_handle: FocusHandle,
    pub(super) camera: Camera,
    camera_fitted: bool,
    pub(super) canvas_bounds: Option<Bounds<Pixels>>,
    pointer_position: Option<Point<f32>>,
    pub(super) hovered_node: Option<usize>,
    interaction: Option<PointerInteraction>,
    simulation: Simulation,
    generation: u64,
    build_task: Task<()>,
}

impl GraphViewState {
    pub(super) fn new(
        input: Entity<InputState>,
        catalog: Option<VaultCatalog>,
        handler: WeakEntity<FileHandler>,
        cx: &Context<Self>,
    ) -> Self {
        Self {
            input,
            catalog,
            handler,
            status: ViewerStatus::Loading,
            focus_handle: cx.focus_handle(),
            camera: Camera::default(),
            camera_fitted: false,
            canvas_bounds: None,
            pointer_position: None,
            hovered_node: None,
            interaction: None,
            simulation: Simulation::default(),
            generation: 0,
            build_task: Task::ready(()),
        }
    }

    pub(super) fn rebuild(&mut self, cx: &mut Context<Self>) {
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.status = ViewerStatus::Loading;
        self.camera = Camera::default();
        self.camera_fitted = false;
        self.pointer_position = None;
        self.hovered_node = None;
        self.interaction = None;
        self.simulation = Simulation::default();
        let source = self.input.read(cx).value().to_string();
        let definition = match crate::document::graph::parse_definition(&source) {
            Ok(definition) => definition,
            Err(error) => {
                self.status = ViewerStatus::Error(error.to_string());
                cx.notify();
                return;
            }
        };
        let Some(catalog) = self.catalog.clone() else {
            self.status = ViewerStatus::Error("No Vault Catalog is available".into());
            cx.notify();
            return;
        };

        self.build_task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { load_snapshot(definition, catalog).await })
                .await;
            let _ = this.update(cx, |state, cx| {
                if state.generation != generation {
                    return;
                }
                state.status = match result {
                    Ok(snapshot) if snapshot.nodes.is_empty() => ViewerStatus::Empty,
                    Ok(snapshot) => ViewerStatus::Ready(snapshot),
                    Err(error) => ViewerStatus::Error(error.to_string()),
                };
                state.simulation = Simulation::default();
                state.camera_fitted = false;
                cx.notify();
            });
        });
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
            && let ViewerStatus::Ready(snapshot) = &self.status
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
        let node = match &mut self.status {
            ViewerStatus::Ready(snapshot) => {
                let node = hit_test_nodes(&snapshot.nodes, world);
                if let Some(index) = node
                    && let Some(node) = snapshot.nodes.get_mut(index)
                {
                    node.velocity = Point::default();
                }
                node
            }
            _ => None,
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
                    if let ViewerStatus::Ready(snapshot) = &mut self.status
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
        let hovered = match &self.status {
            ViewerStatus::Ready(snapshot) => hit_test_nodes(&snapshot.nodes, world),
            _ => None,
        };
        if hovered != self.hovered_node {
            self.hovered_node = hovered;
            cx.notify();
        }
    }

    fn handle_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        _window: &mut Window,
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
            match &mut self.status {
                ViewerStatus::Ready(snapshot) => snapshot.nodes.get_mut(index).map(|node| {
                    node.velocity = Point::default();
                    node.relative_path.to_string_lossy().replace('\\', "/")
                }),
                _ => None,
            }
        } else {
            None
        };
        if !interaction.moved
            && let Some(target) = target
        {
            let new_tab = event.modifiers.platform;
            let _ = self.handler.update(cx, |_handler, cx| {
                cx.emit(FileHandlerEvent::LinkClicked(target, new_tab));
            });
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

    #[allow(clippy::unused_self)]
    pub(super) fn render_centered(
        &self,
        message: impl Into<gpui::SharedString>,
        cx: &App,
    ) -> AnyElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .p_4()
            .text_color(cx.theme().muted_foreground)
            .child(message.into())
            .into_any_element()
    }

    pub(super) fn render_canvas(&mut self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let pinned = self.interaction.and_then(|interaction| interaction.node);
        let hover_query = self
            .interaction
            .is_none()
            .then(|| self.pointer_position.zip(self.viewport()))
            .flatten();
        let ViewerStatus::Ready(snapshot) = &mut self.status else {
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
            .id("graph-viewer")
            .size_full()
            .relative()
            .overflow_hidden()
            .bg(cx.theme().background)
            .track_focus(&self.focus_handle)
            .on_mouse_down(MouseButton::Left, cx.listener(Self::handle_mouse_down))
            .on_mouse_move(cx.listener(Self::handle_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::handle_mouse_up))
            .on_scroll_wheel(cx.listener(Self::handle_scroll))
            .on_prepaint(move |bounds, _window, cx| {
                let _ = entity.update(cx, |state, cx| state.set_canvas_bounds(bounds, cx));
            })
            .child(
                gpui::canvas(
                    move |bounds, _window, _cx| (bounds, snapshot_for_paint, camera_for_paint),
                    move |_bounds, (bounds, snapshot, camera), window, cx| {
                        super::paint::paint_graph(
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
        root.into_any_element()
    }
}
