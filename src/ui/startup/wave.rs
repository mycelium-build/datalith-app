//! The animated wave element used by the startup screen.

use conv::{ConvUtil as _, UnwrapOrInf as _};

use gpui_kit::{Bounds, Hsla, Window, point, px, size};

use super::paint::MonolithElement;

/// concentric rings around the window center whose front has travelled `front` pixels from the center.
pub struct Wave {
    pub(super) center_x: f32,
    pub(super) center_y: f32,
    pub(super) cell: f32,
    pub(super) band: f32,
    pub(super) cols: usize,
    pub(super) rows: usize,
    pub(super) front: f32,
    pub(super) revealing: bool,
    pub(super) alpha: f32,
    pub(super) cover: Hsla,
}

impl Wave {
    pub(super) fn paint_row(&self, row: usize, dy: f32, window: &mut Window) {
        if self.revealing {
            self.paint_reveal_row(row, dy, window);
        } else {
            self.paint_blind_row(row, dy, window);
        }
    }

    /// Column range of cells whose center lies within `radius` of the window center,
    /// for a row at vertical offset `dy`, or `None` when no cell does.
    fn circle_column_range(&self, dy: f32, radius: f32) -> Option<std::ops::Range<usize>> {
        if radius <= 0.0 || dy.abs() >= radius {
            return None;
        }
        let half = (dy.mul_add(-dy, radius * radius)).sqrt();
        let cols = self.cols.approx_as::<f32>().unwrap_or_inf();
        let first = ((self.center_x - half) / self.cell - 0.5)
            .ceil()
            .clamp(0.0, cols)
            .approx_as::<usize>()
            .unwrap_or(0);
        let last = (((self.center_x + half) / self.cell - 0.5).floor() + 1.0)
            .clamp(0.0, cols)
            .approx_as::<usize>()
            .unwrap_or(0);
        (first < last).then_some(first..last)
    }

    /// The blind (`revealing = false`) front submerges everything within the front radius:
    /// a full-color interior plus a ramping band at the edge.
    pub(super) fn paint_blind_row(&self, row: usize, dy: f32, window: &mut Window) {
        let Some(painted) = self.circle_column_range(dy, self.front) else {
            return;
        };
        match self.circle_column_range(dy, self.front - self.band) {
            Some(interior) => {
                Self::paint_row_strip(
                    row,
                    self.cell,
                    interior.start,
                    interior.end,
                    self.cover,
                    window,
                );
                self.paint_band_cells(row, dy, &(painted.start..interior.start), window);
                self.paint_band_cells(row, dy, &(interior.end..painted.end), window);
            }
            None => {
                self.paint_band_cells(row, dy, &painted, window);
            }
        }
    }

    /// The reveal (`revealing = true`) front is a hole that grows from the center:
    /// the full-color exterior is painted as strip quads and only the ramping band around the hole is painted cell-by-cell.
    pub(super) fn paint_reveal_row(&self, row: usize, dy: f32, window: &mut Window) {
        let revealed = self.circle_column_range(dy, self.front);
        match self.circle_column_range(dy, self.front + self.band) {
            None => {
                Self::paint_row_strip(row, self.cell, 0, self.cols, self.cover, window);
            }
            Some(full) => {
                Self::paint_row_strip(row, self.cell, 0, full.start, self.cover, window);
                Self::paint_row_strip(row, self.cell, full.end, self.cols, self.cover, window);
                match &revealed {
                    Some(revealed) => {
                        self.paint_band_cells(row, dy, &(full.start..revealed.start), window);
                        self.paint_band_cells(row, dy, &(revealed.end..full.end), window);
                    }
                    None => {
                        self.paint_band_cells(row, dy, &full, window);
                    }
                }
            }
        }
    }

    pub(super) fn paint_band_cells(
        &self,
        row: usize,
        dy: f32,
        columns: &std::ops::Range<usize>,
        window: &mut Window,
    ) {
        for col in columns.clone() {
            let x = self
                .cell
                .mul_add(col.approx_as::<f32>().unwrap_or_inf() + 0.5, 0.0);
            let dx = x - self.center_x;
            let distance = dx.mul_add(dx, dy * dy).sqrt();
            let offset = if self.revealing {
                distance - self.front
            } else {
                self.front - distance
            };
            if offset <= 0.0 {
                continue;
            }
            let color = if offset <= self.cell {
                self.cover.opacity(self.alpha * 0.3)
            } else if offset <= self.cell * 2.0 {
                self.cover.opacity(self.alpha * 0.65)
            } else {
                self.cover
            };
            let x = self.cell * col.approx_as::<f32>().unwrap_or_inf();
            let y = self.cell * row.approx_as::<f32>().unwrap_or_inf();
            MonolithElement::fill(
                Bounds::new(point(px(x), px(y)), size(px(self.cell), px(self.cell))),
                color,
                window,
            );
        }
    }

    pub(super) fn paint_row_strip(
        row: usize,
        cell: f32,
        from_col: usize,
        to_col: usize,
        color: Hsla,
        window: &mut Window,
    ) {
        if from_col >= to_col {
            return;
        }
        let x = cell * from_col.approx_as::<f32>().unwrap_or_inf();
        let y = cell * row.approx_as::<f32>().unwrap_or_inf();
        let width = cell
            * to_col
                .saturating_sub(from_col)
                .approx_as::<f32>()
                .unwrap_or_inf();
        MonolithElement::fill(
            Bounds::new(point(px(x), px(y)), size(px(width), px(cell))),
            color,
            window,
        );
    }
}
