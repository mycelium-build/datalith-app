use gpui::{Point, point};

use super::model::{GraphSnapshot, deterministic_position};

const VELOCITY_DAMPING: f32 = 0.64;
const COOLING: f32 = 0.996;
const SLEEP_ALPHA: f32 = 0.002;
const BARNES_HUT_THETA: f32 = 1.0;
const MAX_ACCELERATION: f32 = 4.0;
pub(super) const MAX_VELOCITY: f32 = 16.0;

pub(super) fn vector_length(vector: Point<f32>) -> f32 {
    (vector.x * vector.x + vector.y * vector.y).sqrt()
}

pub(super) fn clamp_magnitude(vector: Point<f32>, maximum: f32) -> Point<f32> {
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

pub(super) fn link_force_scale(degree: usize) -> f32 {
    1.0 / degree.max(1) as f32
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Simulation {
    alpha: f32,
}

impl Default for Simulation {
    fn default() -> Self {
        Self { alpha: 1.0 }
    }
}

impl Simulation {
    pub(super) fn step(&mut self, snapshot: &mut GraphSnapshot, pinned: Option<usize>) {
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

    pub(super) fn reheat(&mut self) {
        self.alpha = 1.0;
    }

    pub(super) fn is_sleeping(&self) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::model::{INITIAL_LAYOUT_REFERENCE_NODES, INITIAL_LAYOUT_RADIUS};
    use super::super::snapshot::make_snapshot;

    #[test]
    fn simulation_limits_acceleration_and_velocity() {
        let acceleration = clamp_magnitude(gpui::point(300.0, 400.0), 4.0);
        let velocity = clamp_magnitude(gpui::point(-300.0, 400.0), MAX_VELOCITY);

        assert!((vector_length(acceleration) - 4.0).abs() < 0.001);
        assert!((vector_length(velocity) - MAX_VELOCITY).abs() < 0.001);
    }

    #[test]
    fn link_force_is_normalized_for_each_endpoint_degree() {
        assert_eq!(link_force_scale(0), 1.0);
        assert_eq!(link_force_scale(1), 1.0);
        assert_eq!(link_force_scale(100), 0.01);
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
        snapshot.nodes[0].position = gpui::point(-200.0, 0.0);
        snapshot.nodes[1].position = gpui::point(200.0, 0.0);
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
        snapshot.nodes[0].position = gpui::point(120.0, 0.0);

        Simulation::default().step(&mut snapshot, None);

        assert_eq!(snapshot.nodes[0].position, gpui::point(120.0, 0.0));
        assert_eq!(snapshot.nodes[0].velocity, std::default::Default::default());
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
        snapshot.nodes[0].position = gpui::point(120.0, 0.0);
        let mut simulation = Simulation::default();

        simulation.step(&mut snapshot, Some(0));
        assert_eq!(snapshot.nodes[0].position, gpui::point(120.0, 0.0));

        simulation.reheat();
        simulation.step(&mut snapshot, None);
        assert!(snapshot.nodes[0].position.x < 120.0);
    }
}
