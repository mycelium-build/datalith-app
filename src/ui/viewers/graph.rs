use gpui::{
    AnyElement, App, AppContext, BorderStyle, Bounds, Context, Corners, Edges, Entity, FocusHandle,
    Hsla, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement, PathBuilder, Pixels, Point, Render, Rgba, ScrollDelta,
    ScrollWheelEvent, Styled, Task, WeakEntity, Window, canvas, div, point, px, quad, size,
};
use gpui_component::input::InputState;
use gpui_component::{ActiveTheme, ElementExt};

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context as _, Result, anyhow};

use crate::document::graph::{
    BorderStyle as GraphBorderStyle, GraphColor, GraphDefinition, GraphEdge, GraphNode,
    GraphPhysics, deduplicate_edges, matching_group, select_nodes,
};
use crate::document::handler::{FileHandler, FileHandlerEvent};
use crate::document::markdown::properties_from_markdown;
use crate::vault::VaultCatalog;

const INITIAL_LAYOUT_RADIUS: f32 = 256.0;
const INITIAL_LAYOUT_REFERENCE_NODES: f32 = 512.0;

fn deterministic_position(path: &str, node_count: usize) -> Point<f32> {
    // FNV-1a is stable across processes, unlike Rust's randomized default hasher.
    let hash = path
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
    let angle = (hash as u32) as f32 / u32::MAX as f32 * std::f32::consts::TAU;
    let population_scale = ((node_count as f32).max(INITIAL_LAYOUT_REFERENCE_NODES)
        / INITIAL_LAYOUT_REFERENCE_NODES)
        .sqrt();
    let radial_hash = ((hash >> 32) as u32) as f32 / u32::MAX as f32;
    let radius = (40.0 + radial_hash * (INITIAL_LAYOUT_RADIUS - 40.0)) * population_scale;
    point(angle.cos() * radius, angle.sin() * radius)
}

const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 8.0;

#[derive(Clone, Copy, Debug)]
struct Camera {
    pan: Point<f32>,
    zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            pan: Point::default(),
            zoom: 1.0,
        }
    }
}

impl Camera {
    fn world_to_screen(&self, world: Point<f32>, viewport: Point<f32>) -> Point<f32> {
        point(
            viewport.x / 2.0 + self.pan.x + world.x * self.zoom,
            viewport.y / 2.0 + self.pan.y + world.y * self.zoom,
        )
    }

    fn screen_to_world(&self, screen: Point<f32>, viewport: Point<f32>) -> Point<f32> {
        point(
            (screen.x - viewport.x / 2.0 - self.pan.x) / self.zoom,
            (screen.y - viewport.y / 2.0 - self.pan.y) / self.zoom,
        )
    }

    fn zoom_at(&mut self, zoom: f32, pointer: Point<f32>, viewport: Point<f32>) {
        let world = self.screen_to_world(pointer, viewport);
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan = point(
            pointer.x - viewport.x / 2.0 - world.x * self.zoom,
            pointer.y - viewport.y / 2.0 - world.y * self.zoom,
        );
    }

    fn fit(&mut self, nodes: &[ViewNode], viewport: Point<f32>) {
        let Some(first) = nodes.first() else {
            *self = Self::default();
            return;
        };
        let (mut min_x, mut max_x) = (
            first.position.x - first.radius,
            first.position.x + first.radius,
        );
        let (mut min_y, mut max_y) = (
            first.position.y - first.radius,
            first.position.y + first.radius,
        );
        for node in &nodes[1..] {
            min_x = min_x.min(node.position.x - node.radius);
            max_x = max_x.max(node.position.x + node.radius);
            min_y = min_y.min(node.position.y - node.radius);
            max_y = max_y.max(node.position.y + node.radius);
        }
        let available_width = (viewport.x - 64.0).max(1.0);
        let available_height = (viewport.y - 64.0).max(1.0);
        let content_width = (max_x - min_x).max(1.0);
        let content_height = (max_y - min_y).max(1.0);
        self.zoom = (available_width / content_width)
            .min(available_height / content_height)
            .clamp(MIN_ZOOM, 1.5);
        self.pan = point(
            -(min_x + max_x) / 2.0 * self.zoom,
            -(min_y + max_y) / 2.0 * self.zoom,
        );
    }
}

const BASE_NODE_RADIUS: f32 = 4.0;
const LINK_SIZE_LOG_STRENGTH: f32 = 0.8;
const MAX_LINK_SIZE_SCALE: f32 = 4.0;

fn incoming_link_scale(incoming: usize) -> f32 {
    (1.0 + (incoming as f32).ln_1p() * LINK_SIZE_LOG_STRENGTH).min(MAX_LINK_SIZE_SCALE)
}

#[derive(Clone, Debug)]
struct ViewNode {
    relative_path: PathBuf,
    label: String,
    orphan: bool,
    color: Option<GraphColor>,
    border_color: Option<GraphColor>,
    border_width: f32,
    hover_color: Option<GraphColor>,
    hover_size: f32,
    hover_border_color: Option<GraphColor>,
    hover_border_width: f32,
    radius: f32,
    center_weight: f32,
    position: Point<f32>,
    velocity: Point<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ViewEdge {
    source: usize,
    target: usize,
}

#[derive(Clone, Debug)]
struct GraphSnapshot {
    nodes: Vec<ViewNode>,
    edges: Vec<ViewEdge>,
    edge_color: Option<GraphColor>,
    edge_width: f32,
    arrows: bool,
    arrow_color: Option<GraphColor>,
    physics: GraphPhysics,
}

fn border_width(style: &GraphBorderStyle) -> f32 {
    style
        .width
        .unwrap_or_else(|| if style.color.is_some() { 1.0 } else { 0.0 })
}

fn hover_border_width(normal: &GraphBorderStyle, hover: &GraphBorderStyle) -> f32 {
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

fn hit_test_nodes(nodes: &[ViewNode], world: Point<f32>) -> Option<usize> {
    nodes.iter().enumerate().rev().find_map(|(index, node)| {
        let dx = world.x - node.position.x;
        let dy = world.y - node.position.y;
        (dx * dx + dy * dy <= node.radius * node.radius).then_some(index)
    })
}

fn make_snapshot(
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

    if !definition.display.orphans.show {
        nodes.retain(|node| connected.contains(&node.path));
        let visible: HashSet<_> = nodes.iter().map(|node| node.path.clone()).collect();
        edges.retain(|edge| visible.contains(&edge.source) && visible.contains(&edge.target));
    }

    let indices: HashMap<_, _> = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.path.clone(), index))
        .collect();
    let view_edges: Vec<_> = edges
        .iter()
        .filter_map(|edge| {
            Some(ViewEdge {
                source: *indices.get(&edge.source)?,
                target: *indices.get(&edge.target)?,
            })
        })
        .collect();
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
            let (style, color, size, proportional) = if orphan {
                let style = &definition.display.orphans.node;
                (
                    style,
                    style.color,
                    style.size.unwrap_or(1.0),
                    style.propertional,
                )
            } else {
                let style = &definition.display.node;
                let group = matching_group(definition, &node.path, &node.properties);
                (
                    style,
                    group.and_then(|group| group.color).or(style.color),
                    style.size.unwrap_or(1.0) * group.and_then(|group| group.size).unwrap_or(1.0),
                    style.propertional,
                )
            };
            let degree_scale = if proportional {
                incoming_link_scale(incoming[index])
            } else {
                1.0
            };
            let radius = BASE_NODE_RADIUS * size * degree_scale;
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
                color,
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

    Ok(GraphSnapshot {
        nodes,
        edges: view_edges,
        edge_color: definition.display.edge.color,
        edge_width: definition.display.edge.width.unwrap_or(1.0),
        arrows: definition.display.arrows.show,
        arrow_color: definition.display.arrows.color,
        physics: definition.physics,
    })
}

const VELOCITY_DAMPING: f32 = 0.64;
const COOLING: f32 = 0.9999;
const SLEEP_ALPHA: f32 = 0.002;
const BARNES_HUT_THETA: f32 = 0.64;
const MAX_ACCELERATION: f32 = 4.0;
const MAX_VELOCITY: f32 = 16.0;

fn vector_length(vector: Point<f32>) -> f32 {
    (vector.x * vector.x + vector.y * vector.y).sqrt()
}

fn clamp_magnitude(vector: Point<f32>, maximum: f32) -> Point<f32> {
    let length = vector_length(vector);
    if !length.is_finite() {
        return Point::default();
    }
    if length <= maximum || length == 0.0 {
        vector
    } else {
        point(vector.x * maximum / length, vector.y * maximum / length)
    }
}

fn link_force_scale(degree: usize) -> f32 {
    1.0 / degree.max(1) as f32
}

#[derive(Clone, Copy, Debug)]
struct Simulation {
    alpha: f32,
}

impl Default for Simulation {
    fn default() -> Self {
        Self { alpha: 1.0 }
    }
}

impl Simulation {
    fn step(&mut self, snapshot: &mut GraphSnapshot, pinned: Option<usize>) {
        if self.is_sleeping() || snapshot.nodes.is_empty() {
            return;
        }

        let positions: Vec<_> = snapshot.nodes.iter().map(|node| node.position).collect();
        let tree = QuadTree::new(&positions);
        let physics = snapshot.physics;
        let mut forces: Vec<Point<f32>> = vec![Point::default(); snapshot.nodes.len()];
        let mut link_degrees = vec![0_usize; snapshot.nodes.len()];
        for edge in &snapshot.edges {
            link_degrees[edge.source] += 1;
            link_degrees[edge.target] += 1;
        }

        for (index, node) in snapshot.nodes.iter().enumerate() {
            let repel = tree.repulsion(index, node.position, physics.repulsion.strength);
            forces[index].x += repel.x;
            forces[index].y += repel.y;
            forces[index].x -= node.position.x * physics.center.strength * node.center_weight;
            forces[index].y -= node.position.y * physics.center.strength * node.center_weight;
        }

        for edge in &snapshot.edges {
            let source = snapshot.nodes[edge.source].position;
            let target = snapshot.nodes[edge.target].position;
            let delta = point(target.x - source.x, target.y - source.y);
            let distance = (delta.x * delta.x + delta.y * delta.y).sqrt().max(0.001);
            let magnitude = (distance - physics.link.distance) * physics.link.strength;
            let force = point(
                delta.x / distance * magnitude,
                delta.y / distance * magnitude,
            );
            let source_scale = link_force_scale(link_degrees[edge.source]);
            let target_scale = link_force_scale(link_degrees[edge.target]);
            forces[edge.source].x += force.x * source_scale;
            forces[edge.source].y += force.y * source_scale;
            forces[edge.target].x -= force.x * target_scale;
            forces[edge.target].y -= force.y * target_scale;
        }

        let node_count = snapshot.nodes.len();
        for (index, (node, force)) in snapshot.nodes.iter_mut().zip(forces).enumerate() {
            if pinned == Some(index) {
                node.velocity = Point::default();
                continue;
            }
            let acceleration = clamp_magnitude(
                point(force.x * self.alpha, force.y * self.alpha),
                MAX_ACCELERATION,
            );
            node.velocity = clamp_magnitude(
                point(
                    (node.velocity.x + acceleration.x) * VELOCITY_DAMPING,
                    (node.velocity.y + acceleration.y) * VELOCITY_DAMPING,
                ),
                MAX_VELOCITY,
            );
            node.position.x += node.velocity.x;
            node.position.y += node.velocity.y;
            if !node.position.x.is_finite() || !node.position.y.is_finite() {
                node.position =
                    deterministic_position(&node.relative_path.to_string_lossy(), node_count);
                node.velocity = Point::default();
            }
        }

        self.alpha *= COOLING;
    }

    fn reheat(&mut self) {
        self.alpha = 1.0;
    }

    fn is_sleeping(&self) -> bool {
        self.alpha <= SLEEP_ALPHA
    }
}

#[derive(Clone, Debug)]
struct QuadNode {
    center: Point<f32>,
    half_size: f32,
    mass: f32,
    mass_center: Point<f32>,
    body: Option<usize>,
    children: [Option<usize>; 4],
}

impl QuadNode {
    fn new(center: Point<f32>, half_size: f32) -> Self {
        Self {
            center,
            half_size,
            mass: 0.0,
            mass_center: Point::default(),
            body: None,
            children: [None; 4],
        }
    }

    fn contains(&self, point: Point<f32>) -> bool {
        point.x >= self.center.x - self.half_size
            && point.x <= self.center.x + self.half_size
            && point.y >= self.center.y - self.half_size
            && point.y <= self.center.y + self.half_size
    }
}

#[derive(Clone, Debug)]
struct QuadTree {
    nodes: Vec<QuadNode>,
    positions: Vec<Point<f32>>,
}

impl QuadTree {
    fn new(positions: &[Point<f32>]) -> Self {
        if positions.is_empty() {
            return Self {
                nodes: Vec::new(),
                positions: Vec::new(),
            };
        }
        let (mut min_x, mut max_x, mut min_y, mut max_y) = (
            positions[0].x,
            positions[0].x,
            positions[0].y,
            positions[0].y,
        );
        for position in &positions[1..] {
            min_x = min_x.min(position.x);
            max_x = max_x.max(position.x);
            min_y = min_y.min(position.y);
            max_y = max_y.max(position.y);
        }
        let center = point((min_x + max_x) / 2.0, (min_y + max_y) / 2.0);
        let half_size = ((max_x - min_x).max(max_y - min_y) / 2.0 + 1.0).max(1.0);
        let mut tree = Self {
            nodes: Vec::with_capacity(positions.len().saturating_mul(2).max(1)),
            positions: positions.to_vec(),
        };
        tree.nodes.push(QuadNode::new(center, half_size));
        for body in 0..positions.len() {
            tree.insert(0, body, 0);
        }
        tree
    }

    fn insert(&mut self, node_index: usize, body: usize, depth: usize) {
        let position = self.positions[body];
        let (existing, has_children, center, half_size) = {
            let node = &mut self.nodes[node_index];
            let new_mass = node.mass + 1.0;
            node.mass_center = point(
                (node.mass_center.x * node.mass + position.x) / new_mass,
                (node.mass_center.y * node.mass + position.y) / new_mass,
            );
            node.mass = new_mass;
            (
                node.body,
                node.children.iter().any(Option::is_some),
                node.center,
                node.half_size,
            )
        };

        if !has_children && existing.is_none() {
            self.nodes[node_index].body = Some(body);
            return;
        }
        if depth >= 20 || half_size <= 0.001 {
            self.nodes[node_index].body = None;
            return;
        }
        if !has_children {
            self.nodes[node_index].body = None;
            if let Some(existing) = existing {
                self.insert_into_child(node_index, existing, depth + 1, center, half_size);
            }
        }
        self.insert_into_child(node_index, body, depth + 1, center, half_size);
    }

    fn insert_into_child(
        &mut self,
        parent: usize,
        body: usize,
        depth: usize,
        center: Point<f32>,
        half_size: f32,
    ) {
        let position = self.positions[body];
        let right = usize::from(position.x >= center.x);
        let bottom = usize::from(position.y >= center.y);
        let quadrant = bottom * 2 + right;
        let child = if let Some(child) = self.nodes[parent].children[quadrant] {
            child
        } else {
            let child_half = half_size / 2.0;
            let child_center = point(
                center.x + if right == 1 { child_half } else { -child_half },
                center.y + if bottom == 1 { child_half } else { -child_half },
            );
            let child = self.nodes.len();
            self.nodes.push(QuadNode::new(child_center, child_half));
            self.nodes[parent].children[quadrant] = Some(child);
            child
        };
        self.insert(child, body, depth);
    }

    fn repulsion(&self, body: usize, position: Point<f32>, strength: f32) -> Point<f32> {
        if self.nodes.is_empty() {
            Point::default()
        } else {
            self.repulsion_from(0, body, position, strength)
        }
    }

    fn repulsion_from(
        &self,
        node_index: usize,
        body: usize,
        position: Point<f32>,
        strength: f32,
    ) -> Point<f32> {
        let node = &self.nodes[node_index];
        if node.mass == 0.0 || node.body == Some(body) {
            return Point::default();
        }
        let mut delta = point(
            position.x - node.mass_center.x,
            position.y - node.mass_center.y,
        );
        if delta.x * delta.x + delta.y * delta.y < 0.000_001 {
            let angle =
                (body as f32 * 2.399_963_1 + node_index as f32).rem_euclid(std::f32::consts::TAU);
            delta = point(angle.cos() * 0.01, angle.sin() * 0.01);
        }
        let distance_squared = delta.x * delta.x + delta.y * delta.y + 16.0;
        let distance = distance_squared.sqrt();
        let is_leaf = node.children.iter().all(Option::is_none);
        if is_leaf
            || (!node.contains(position) && node.half_size * 2.0 / distance < BARNES_HUT_THETA)
        {
            let magnitude = strength * node.mass / distance_squared;
            return point(
                delta.x / distance * magnitude,
                delta.y / distance * magnitude,
            );
        }

        node.children
            .iter()
            .flatten()
            .fold(Point::default(), |mut force, child| {
                let child_force = self.repulsion_from(*child, body, position, strength);
                force.x += child_force.x;
                force.y += child_force.y;
                force
            })
    }
}

fn load_snapshot(definition: GraphDefinition, catalog: VaultCatalog) -> Result<GraphSnapshot> {
    let root = catalog.root();
    let mut candidates = Vec::new();
    for absolute_path in catalog.tracked_paths().into_iter().filter(|path| {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
    }) {
        let relative_path = absolute_path
            .strip_prefix(&root)
            .map(PathBuf::from)
            .map_err(|_| {
                anyhow!(
                    "tracked file is outside the Vault: {}",
                    absolute_path.display()
                )
            })?;
        let source = match fs::read_to_string(&absolute_path) {
            Ok(source) => source,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to read Markdown file {}", relative_path.display())
                });
            }
        };
        candidates.push(GraphNode {
            path: relative_path,
            properties: properties_from_markdown(&source),
        });
    }

    let edges = catalog.wiki_link_edges().into_iter().filter_map(|edge| {
        Some(GraphEdge {
            source: edge.source.strip_prefix(&root).ok()?.to_path_buf(),
            target: edge.target.strip_prefix(&root).ok()?.to_path_buf(),
        })
    });
    make_snapshot(&definition, candidates, edges)
}

#[derive(Debug)]
enum ViewerStatus {
    Loading,
    Ready(GraphSnapshot),
    Empty,
    Error(String),
}

#[derive(Clone, Copy, Debug)]
struct PointerInteraction {
    start: Point<f32>,
    last: Point<f32>,
    node: Option<usize>,
    moved: bool,
}

struct GraphViewState {
    input: Entity<InputState>,
    catalog: Option<VaultCatalog>,
    handler: WeakEntity<FileHandler>,
    status: ViewerStatus,
    focus_handle: FocusHandle,
    camera: Camera,
    camera_fitted: bool,
    canvas_bounds: Option<Bounds<Pixels>>,
    pointer_position: Option<Point<f32>>,
    hovered_node: Option<usize>,
    interaction: Option<PointerInteraction>,
    simulation: Simulation,
    generation: u64,
    build_task: Task<()>,
}

impl GraphViewState {
    fn new(
        input: Entity<InputState>,
        catalog: Option<VaultCatalog>,
        handler: WeakEntity<FileHandler>,
        cx: &mut Context<Self>,
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

    fn rebuild(&mut self, cx: &mut Context<Self>) {
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
            while !catalog.initialization_complete() {
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
            }
            let result = cx
                .background_spawn(async move { load_snapshot(definition, catalog) })
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
                f32::from(position.x - bounds.origin.x),
                f32::from(position.y - bounds.origin.y),
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
                if let Some(index) = node {
                    snapshot.nodes[index].velocity = Point::default();
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
            let dx = local.x - interaction.last.x;
            let dy = local.y - interaction.last.y;
            let total_dx = local.x - interaction.start.x;
            let total_dy = local.y - interaction.start.y;
            let was_moved = interaction.moved;
            interaction.moved |= total_dx * total_dx + total_dy * total_dy > 16.0;
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
                    self.camera.pan.x += dx;
                    self.camera.pan.y += dy;
                } else {
                    self.camera.pan.x += total_dx;
                    self.camera.pan.y += total_dy;
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

    fn render_centered(&self, message: impl Into<gpui::SharedString>, cx: &App) -> AnyElement {
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

    fn render_canvas(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let pinned = self.interaction.and_then(|interaction| interaction.node);
        let hover_query = self
            .interaction
            .is_none()
            .then(|| self.pointer_position.zip(self.viewport()))
            .flatten();
        let ViewerStatus::Ready(snapshot) = &mut self.status else {
            unreachable!();
        };
        self.simulation.step(snapshot, pinned);
        if let Some((pointer, viewport)) = hover_query {
            let world = self.camera.screen_to_world(pointer, viewport);
            self.hovered_node = hit_test_nodes(&snapshot.nodes, world);
        }
        if !self.simulation.is_sleeping() {
            window.request_animation_frame();
        }

        let snapshot_for_paint = snapshot.clone();
        let camera_for_paint = self.camera;
        let hovered_for_paint = self.hovered_node;
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
                canvas(
                    move |bounds, _window, _cx| (bounds, snapshot_for_paint, camera_for_paint),
                    move |_bounds, (bounds, snapshot, camera), window, cx| {
                        paint_graph(bounds, &snapshot, camera, hovered_for_paint, window, cx);
                    },
                )
                .absolute()
                .size_full(),
            );

        if let (Some(index), Some(bounds)) = (self.hovered_node, self.canvas_bounds)
            && let Some(node) = snapshot.nodes.get(index)
        {
            let viewport = point(f32::from(bounds.size.width), f32::from(bounds.size.height));
            let screen = self.camera.world_to_screen(node.position, viewport);
            root = root.child(
                div()
                    .absolute()
                    .left(px(screen.x + 12.0))
                    .top(px(screen.y + 12.0))
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(cx.theme().muted)
                    .text_color(cx.theme().foreground)
                    .child(node.label.clone()),
            );
        }
        root.into_any_element()
    }
}

impl Render for GraphViewState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.status {
            ViewerStatus::Loading => self.render_centered("Loading Graph View…", cx),
            ViewerStatus::Empty => {
                self.render_centered("No Markdown files match this Graph Definition.", cx)
            }
            ViewerStatus::Error(error) => self.render_centered(error.clone(), cx),
            ViewerStatus::Ready(_) => self.render_canvas(window, cx),
        }
    }
}

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

fn paint_graph(
    bounds: Bounds<Pixels>,
    snapshot: &GraphSnapshot,
    camera: Camera,
    hovered_node: Option<usize>,
    window: &mut Window,
    cx: &mut App,
) {
    let viewport = point(f32::from(bounds.size.width), f32::from(bounds.size.height));
    let edge_color = snapshot
        .edge_color
        .map(graph_color)
        .unwrap_or_else(|| cx.theme().border.opacity(0.65));
    let mut edge_builder = PathBuilder::stroke(px((snapshot.edge_width * camera.zoom).max(0.35)));
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
        edge_builder.move_to(source);
        edge_builder.line_to(target);
    }
    if let Ok(path) = edge_builder.build() {
        window.paint_path(path, edge_color);
    }

    if snapshot.arrows {
        let arrow_color = snapshot.arrow_color.map(graph_color).unwrap_or(edge_color);
        let mut arrows = PathBuilder::fill();
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
            window.paint_path(path, arrow_color);
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
            node.hover_color.map(graph_color).unwrap_or(base_color)
        } else {
            base_color
        };
        let (border_color, border_width) = if hovered {
            (node.hover_border_color, node.hover_border_width)
        } else {
            (node.border_color, node.border_width)
        };
        let border_color = border_color.map(graph_color).unwrap_or(color);
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

pub(crate) struct GraphViewer {
    state: Entity<GraphViewState>,
}

impl GraphViewer {
    pub(crate) fn new(
        input: Entity<InputState>,
        catalog: Option<VaultCatalog>,
        cx: &mut Context<FileHandler>,
    ) -> Self {
        let handler = cx.entity().downgrade();
        let state = cx.new(|cx| GraphViewState::new(input, catalog, handler, cx));
        state.update(cx, |state, cx| state.rebuild(cx));
        Self { state }
    }

    pub(crate) fn refresh(&self, cx: &mut App) {
        self.state.update(cx, |state, cx| state.rebuild(cx));
    }

    pub(crate) fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.state.read(cx).focus_handle.clone()
    }

    pub(crate) fn render(&self, _handler: Entity<FileHandler>, _cx: &mut App) -> AnyElement {
        self.state.clone().into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph_catalog(root: &std::path::Path) -> VaultCatalog {
        use crate::document::file_types::{FileTypeCapabilities, RegisteredFileTypes};

        let types = RegisteredFileTypes::new([
            (
                "md".to_string(),
                FileTypeCapabilities {
                    text_search: true,
                    wiki_links: true,
                },
            ),
            (
                "graph".to_string(),
                FileTypeCapabilities {
                    text_search: false,
                    wiki_links: false,
                },
            ),
            (
                "todotxt".to_string(),
                FileTypeCapabilities {
                    text_search: true,
                    wiki_links: false,
                },
            ),
        ]);
        let catalog = VaultCatalog::open(root.to_path_buf(), types).unwrap();
        for _ in 0..200 {
            if catalog.initialization_complete() {
                return catalog;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("Vault Catalog did not finish initialization");
    }

    fn graph_test_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("datalith-graph-{name}-{}", std::process::id()))
    }

    #[test]
    fn node_paths_produce_deterministic_distinct_starting_positions() {
        let node_count = INITIAL_LAYOUT_REFERENCE_NODES as usize;
        let first = deterministic_position("Inbox/day.md", node_count);
        let again = deterministic_position("Inbox/day.md", node_count);
        let other = deterministic_position("Projects/day.md", node_count);

        assert_eq!(first, again);
        assert_ne!(first, other);
        assert!(first.x.is_finite() && first.y.is_finite());
    }

    #[test]
    fn initial_layout_area_grows_with_node_count() {
        let reference_node_count = INITIAL_LAYOUT_REFERENCE_NODES as usize;
        let small = deterministic_position("Inbox/day.md", reference_node_count);
        let large = deterministic_position("Inbox/day.md", reference_node_count * 4);
        let small_radius = (small.x * small.x + small.y * small.y).sqrt();
        let large_radius = (large.x * large.x + large.y * large.y).sqrt();

        assert!((large_radius / small_radius - 2.0).abs() < 0.001);
    }

    #[test]
    fn simulation_limits_acceleration_and_velocity() {
        let acceleration = clamp_magnitude(point(300.0, 400.0), MAX_ACCELERATION);
        let velocity = clamp_magnitude(point(-300.0, 400.0), MAX_VELOCITY);

        assert!((vector_length(acceleration) - MAX_ACCELERATION).abs() < 0.001);
        assert!((vector_length(velocity) - MAX_VELOCITY).abs() < 0.001);
    }

    #[test]
    fn link_force_is_normalized_for_each_endpoint_degree() {
        assert_eq!(link_force_scale(0), 1.0);
        assert_eq!(link_force_scale(1), 1.0);
        assert_eq!(link_force_scale(100), 0.01);
    }

    #[test]
    fn zoom_keeps_the_world_point_under_the_pointer() {
        let viewport = point(800.0, 600.0);
        let pointer = point(615.0, 210.0);
        let mut camera = Camera::default();
        let before = camera.screen_to_world(pointer, viewport);

        camera.zoom_at(2.0, pointer, viewport);

        let after = camera.screen_to_world(pointer, viewport);
        assert!((before.x - after.x).abs() < 0.001);
        assert!((before.y - after.y).abs() < 0.001);
    }

    #[test]
    fn snapshot_hides_orphans_and_applies_the_first_matching_group() {
        use std::path::PathBuf;

        use crate::document::graph::{GraphEdge, GraphNode, parse_definition};

        let definition = parse_definition(
            r##"
groups:
  - name: Done
    filters: 'status == "done"'
    color: '#ff0000'
display:
  orphans:
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
            source: PathBuf::from("one.md"),
            target: PathBuf::from("two.md"),
        }];

        let snapshot = make_snapshot(&definition, nodes, edges).unwrap();

        assert_eq!(snapshot.nodes.len(), 2);
        let done = snapshot
            .nodes
            .iter()
            .find(|node| node.relative_path == std::path::Path::new("one.md"))
            .unwrap();
        assert!(done.color.is_some());
    }

    #[test]
    fn linked_nodes_converge_without_non_finite_motion() {
        use std::path::PathBuf;

        use crate::document::graph::{GraphEdge, GraphNode, parse_definition};

        let definition = parse_definition("").unwrap();
        let nodes = ["one.md", "two.md"].into_iter().map(|path| GraphNode {
            path: PathBuf::from(path),
            properties: yaml_serde::Value::Mapping(Default::default()),
        });
        let edges = [GraphEdge {
            source: PathBuf::from("one.md"),
            target: PathBuf::from("two.md"),
        }];
        let mut snapshot = make_snapshot(&definition, nodes, edges).unwrap();
        snapshot.nodes[0].position = point(-200.0, 0.0);
        snapshot.nodes[1].position = point(200.0, 0.0);
        let mut simulation = Simulation::default();

        for _ in 0..400 {
            simulation.step(&mut snapshot, None);
        }

        let distance = (snapshot.nodes[1].position.x - snapshot.nodes[0].position.x).abs();
        assert!(distance < 400.0);
        assert!(snapshot.nodes.iter().all(|node| {
            node.position.x.is_finite()
                && node.position.y.is_finite()
                && node.velocity.x.is_finite()
                && node.velocity.y.is_finite()
        }));
    }

    #[test]
    fn large_hub_graph_remains_stable_without_catapulting_nodes() {
        use crate::document::graph::{GraphEdge, GraphNode, parse_definition};

        const NODE_COUNT: usize = INITIAL_LAYOUT_REFERENCE_NODES as usize * 4;

        let definition = parse_definition(&format!("limit: {NODE_COUNT}")).unwrap();
        let nodes = (0..NODE_COUNT).map(|index| GraphNode {
            path: format!("node-{index}.md").into(),
            properties: yaml_serde::Value::Mapping(Default::default()),
        });
        let edges = (1..NODE_COUNT).map(|index| GraphEdge {
            source: format!("node-{index}.md").into(),
            target: "node-0.md".into(),
        });
        let mut snapshot = make_snapshot(&definition, nodes, edges).unwrap();
        let mut simulation = Simulation::default();

        for _ in 0..400 {
            simulation.step(&mut snapshot, None);
            assert!(snapshot.nodes.iter().all(|node| {
                node.position.x.is_finite()
                    && node.position.y.is_finite()
                    && vector_length(node.velocity) <= MAX_VELOCITY + 0.001
            }));
        }

        let layout_scale = (NODE_COUNT as f32 / INITIAL_LAYOUT_REFERENCE_NODES).sqrt();
        let maximum_radius = snapshot
            .nodes
            .iter()
            .map(|node| vector_length(node.position))
            .fold(0.0_f32, f32::max);
        assert!(maximum_radius < INITIAL_LAYOUT_RADIUS * layout_scale * 3.0);
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
                position: point(0.0, 0.0),
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
                position: point(2.0, 0.0),
                velocity: Point::default(),
            },
        ];

        assert_eq!(hit_test_nodes(&nodes, point(1.0, 0.0)), Some(1));
        assert_eq!(hit_test_nodes(&nodes, point(30.0, 0.0)), None);
    }

    #[test]
    fn incoming_links_scale_nodes_and_orphan_style_overrides_groups() {
        use std::path::PathBuf;

        use crate::document::graph::{GraphEdge, GraphNode, parse_definition};

        let definition = parse_definition(
            r##"
groups:
  - name: Everything
    filters: []
    color: '#ff0000'
display:
  orphans:
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
  orphans:
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
    fn configured_physics_controls_simulation_forces() {
        use crate::document::graph::{GraphNode, parse_definition};

        let definition = parse_definition(
            "physics:\n  center:\n    strength: 0\n  repulsion:\n    strength: 0\n  link:\n    strength: 0",
        )
        .unwrap();
        let nodes = [GraphNode {
            path: "still.md".into(),
            properties: yaml_serde::Value::Mapping(Default::default()),
        }];
        let mut snapshot = make_snapshot(&definition, nodes, []).unwrap();
        snapshot.nodes[0].position = point(120.0, 0.0);

        Simulation::default().step(&mut snapshot, None);

        assert_eq!(snapshot.nodes[0].position, point(120.0, 0.0));
        assert_eq!(snapshot.nodes[0].velocity, Point::default());
    }

    #[test]
    fn proportional_node_growth_is_logarithmic_and_capped() {
        assert_eq!(incoming_link_scale(0), 1.0);
        assert!(incoming_link_scale(10) > incoming_link_scale(1));
        assert!(incoming_link_scale(100) > incoming_link_scale(10));
        assert_eq!(incoming_link_scale(usize::MAX), 4.0);
    }

    #[test]
    fn releasing_a_drag_restores_force_motion() {
        use crate::document::graph::{GraphNode, parse_definition};

        let definition = parse_definition("").unwrap();
        let nodes = [GraphNode {
            path: "moving.md".into(),
            properties: yaml_serde::Value::Mapping(Default::default()),
        }];
        let mut snapshot = make_snapshot(&definition, nodes, []).unwrap();
        snapshot.nodes[0].position = point(120.0, 0.0);
        let mut simulation = Simulation::default();

        simulation.step(&mut snapshot, Some(0));
        assert_eq!(snapshot.nodes[0].position, point(120.0, 0.0));

        simulation.reheat();
        simulation.step(&mut snapshot, None);
        assert!(snapshot.nodes[0].position.x < 120.0);
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

        let snapshot = load_snapshot(definition, catalog.clone()).unwrap();

        assert_eq!(snapshot.nodes.len(), 1);
        assert_eq!(
            snapshot.nodes[0].relative_path,
            PathBuf::from("selected.md")
        );
        drop(catalog);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn catalog_snapshot_reports_an_existing_unreadable_markdown_file() {
        let root = graph_test_root("unreadable");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("binary.md"), [0xff, 0xfe, 0xfd]).unwrap();
        let catalog = graph_catalog(&root);
        let definition = crate::document::graph::parse_definition("").unwrap();

        let error = load_snapshot(definition, catalog.clone()).unwrap_err();

        assert!(error.to_string().contains("binary.md"));
        drop(catalog);
        let _ = fs::remove_dir_all(root);
    }
}
