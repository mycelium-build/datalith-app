use gpui::{Point, point};

use super::model::ViewNode;

pub(super) const MIN_ZOOM: f32 = 0.1;
pub(super) const MAX_ZOOM: f32 = 8.0;

#[derive(Clone, Copy, Debug)]
pub(super) struct Camera {
    pub(super) pan: Point<f32>,
    pub(super) zoom: f32,
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
    pub(super) fn world_to_screen(&self, world: Point<f32>, viewport: Point<f32>) -> Point<f32> {
        point(
            viewport.x / 2.0 + self.pan.x + world.x * self.zoom,
            viewport.y / 2.0 + self.pan.y + world.y * self.zoom,
        )
    }

    pub(super) fn screen_to_world(&self, screen: Point<f32>, viewport: Point<f32>) -> Point<f32> {
        point(
            (screen.x - viewport.x / 2.0 - self.pan.x) / self.zoom,
            (screen.y - viewport.y / 2.0 - self.pan.y) / self.zoom,
        )
    }

    pub(super) fn zoom_at(&mut self, zoom: f32, pointer: Point<f32>, viewport: Point<f32>) {
        let world = self.screen_to_world(pointer, viewport);
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan = point(
            pointer.x - viewport.x / 2.0 - world.x * self.zoom,
            pointer.y - viewport.y / 2.0 - world.y * self.zoom,
        );
    }

    pub(super) fn fit(&mut self, nodes: &[ViewNode], viewport: Point<f32>) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_keeps_the_world_point_under_the_pointer() {
        let viewport = gpui::point(800.0, 600.0);
        let pointer = gpui::point(615.0, 210.0);
        let mut camera = Camera::default();
        let before = camera.screen_to_world(pointer, viewport);

        camera.zoom_at(2.0, pointer, viewport);

        let after = camera.screen_to_world(pointer, viewport);
        assert!((before.x - after.x).abs() < 0.001);
        assert!((before.y - after.y).abs() < 0.001);
    }
}
