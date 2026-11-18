//! Custom GPUI element drawing the animated startup monolith and wave.

use gpui_kit::{
    App, BorderStyle, Bounds, Corners, Edges, Element, ElementId, Hsla, IntoElement, LayoutId,
    Length, Pixels, Size, Style, Window, point, px, quad, size, solid_background,
    transparent_black,
};

use super::{GRID_MIN_CELL_PX, Phase, WAVES_PER_IGNITE, ease_in, ease_in_out, ease_out, lerp_hsla};
use conv::{ConvUtil as _, UnwrapOrInf as _};

use crate::ui::monolith::{Cell, LogoGrid, Tier};

pub(super) struct MonolithElement {
    pub(super) phase: Phase,
    pub(super) progress: f32,
    pub(super) elapsed: f32,
    pub(super) bloomed: bool,
    pub(super) primary: Hsla,  // M color
    pub(super) tier_one: Hsla, // 1 color
    pub(super) tier_two: Hsla, // 2 color
    pub(super) tier_inscription: Hsla,
    pub(super) background: Hsla,
    pub(super) logo: LogoGrid,
}

use super::wave::Wave;

impl IntoElement for MonolithElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for MonolithElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&gpui_kit::GlobalElementId>,
        _inspector_id: Option<&gpui_kit::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let size = window.bounds().size;
        let style = Style {
            size: Size::new(
                Length::Definite(size.width.into()),
                Length::Definite(size.height.into()),
            ),
            ..Default::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&gpui_kit::GlobalElementId>,
        _inspector_id: Option<&gpui_kit::InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _id: Option<&gpui_kit::GlobalElementId>,
        _inspector_id: Option<&gpui_kit::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        _cx: &mut App,
    ) {
        match self.phase {
            Phase::Bloom => self.paint_bloom(bounds, window),
            Phase::Dissolve => self.paint_dissolve(bounds, window),
            Phase::Done => {}
            _ => self.paint_monolith(bounds, window),
        }
    }
}

impl MonolithElement {
    pub(super) fn fill(bounds: Bounds<Pixels>, color: Hsla, window: &mut Window) {
        window.paint_quad(quad(
            bounds,
            Corners::default(),
            solid_background(color),
            Edges::default(),
            transparent_black(),
            BorderStyle::default(),
        ));
    }

    fn max_radius(bounds: Bounds<Pixels>) -> f32 {
        bounds
            .size
            .width
            .as_f32()
            .hypot(bounds.size.height.as_f32())
            * 0.5
    }

    fn logo_cell(&self, bounds: Bounds<Pixels>) -> f32 {
        ((bounds.size.height.as_f32() * 0.6) / self.logo.height)
            .floor()
            .clamp(GRID_MIN_CELL_PX, 24.0)
    }

    /// The monolith border pulses through the tiers `white -> 2 -> 1 -> M` cycling,
    /// so the outline reads as a glowing line.
    fn border_pulse(&self) -> Hsla {
        let quarter = 0.25;
        let p = (self.elapsed * 1.1) % 1.0;
        let t = ease_in_out((p / quarter) % 1.0);
        if p < quarter {
            lerp_hsla(self.tier_inscription, self.tier_two, t)
        } else if p < quarter * 2.0 {
            lerp_hsla(self.tier_two, self.tier_one, t)
        } else if p < quarter * 3.0 {
            lerp_hsla(self.tier_one, self.primary, t)
        } else {
            lerp_hsla(self.primary, self.tier_inscription, t)
        }
    }

    /// The uniform color both the border and the inscriptions take during the glow:
    /// every cell transitions `white -> 2 → 1 -> M` together.
    fn uniform_settle(&self, progress: f32) -> Hsla {
        let third = 1.0 / 3.0;
        if progress < third {
            lerp_hsla(self.tier_inscription, self.tier_two, progress * 3.0)
        } else if progress < 2.0 * third {
            lerp_hsla(self.tier_two, self.tier_one, (progress - third) * 3.0)
        } else {
            lerp_hsla(
                self.tier_one,
                self.primary,
                third.mul_add(-2.0, progress) * 3.0,
            )
        }
    }

    /// The border color: it pulses while the waves fall,
    /// then turns to `M` uniformly with the inscriptions during the glow.
    fn border_color(&self) -> Hsla {
        match self.phase {
            Phase::Glow => self.uniform_settle(self.progress),
            Phase::Bloom => self.primary,
            _ => self.border_pulse(),
        }
    }

    /// The color of an inscription cell.
    /// During the ignite phase two waves fall from the top of the monolith down,
    /// each trailing a smooth gradient from `M` up through `1` and `2` into pure white, with a sharp edge at the front;
    /// during the glow the inscriptions turn to `M` uniformly with the border,
    /// and keep `M` through the flash.
    fn inscription_color(&self, cell: &Cell) -> Hsla {
        match self.phase {
            Phase::Ignite => {
                let row_norm = cell.row / self.logo.height;
                let distance = self.progress.mul_add(WAVES_PER_IGNITE, -row_norm);
                if distance < 0.0 {
                    self.tier_inscription
                } else {
                    self.wave_color(distance % 1.0, 1.0 / WAVES_PER_IGNITE)
                }
            }
            Phase::Glow => self.uniform_settle(self.progress),
            Phase::Bloom => self.primary,
            _ => self.tier_inscription,
        }
    }

    /// The smooth gradient trail behind a falling wave:
    /// the front edge is `M`, fading up through `1` and `2` into a trailing pure white.
    fn wave_color(&self, behind: f32, trail: f32) -> Hsla {
        if behind >= trail {
            return self.tier_inscription;
        }
        let third = trail / 3.0;
        let t = ease_in_out((behind / third) % 1.0);
        if behind < third {
            lerp_hsla(self.primary, self.tier_one, t)
        } else if behind < third * 2.0 {
            lerp_hsla(self.tier_one, self.tier_two, t)
        } else {
            lerp_hsla(self.tier_two, self.tier_inscription, t)
        }
    }

    fn paint_monolith(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        Self::fill(bounds, self.background, window);
        let (origin_x, origin_y, cell_size) = self.logo_origin(bounds);
        self.paint_logo_cells(origin_x, origin_y, cell_size, window);
    }

    /// The top-left corner of the logo grid and its pixel size,
    /// accounting for the rise at the start of the animation.
    fn logo_origin(&self, bounds: Bounds<Pixels>) -> (f32, f32, f32) {
        let center = bounds.center();
        let cell_size = self.logo_cell(bounds);
        let logo_width = self.logo.width * cell_size;
        let logo_height = self.logo.height * cell_size;
        let origin_x = logo_width.mul_add(-0.5, center.x.as_f32());
        let mut origin_y = logo_height.mul_add(-0.5, center.y.as_f32());
        if self.phase == Phase::Rise {
            let window_h = bounds.size.height.as_f32();
            let lift = window_h.mul_add(0.5, logo_height) * (1.0 - ease_out(self.progress));
            origin_y += lift;
        }
        (origin_x, origin_y, cell_size)
    }

    fn paint_logo_cells(&self, origin_x: f32, origin_y: f32, cell_size: f32, window: &mut Window) {
        for cell in &self.logo.cells {
            let color = match cell.tier {
                Tier::Light => self.border_color(),
                Tier::Inscription => self.inscription_color(cell),
                Tier::RightSide => self.tier_one,
                Tier::LeftSide => self.tier_two,
                Tier::Top => self.tier_inscription,
            };
            let x = cell.col.mul_add(cell_size, origin_x).round();
            let y = cell.row.mul_add(cell_size, origin_y).round();
            let cell_bounds = Bounds::new(point(px(x), px(y)), size(px(cell_size), px(cell_size)));
            Self::fill(cell_bounds, color, window);
        }
    }

    /// The wave is painted as concentric rings around the window center.
    /// Only the thin band where the color depends on the front is painted cell-by-cell;
    /// the uniformly colored interior/exterior is painted as one strip quad per row,
    /// so a large window stays cheap during the ~2s wave.
    ///
    /// `cover` is the color the wave paints over the covered region
    /// (the accent color for the bloom blind, and either the accent or the theme background for the dissolve).
    fn paint_wave(
        &self,
        bounds: Bounds<Pixels>,
        front: f32,
        revealing: bool,
        alpha: f32,
        cover: Hsla,
        window: &mut Window,
    ) {
        let center = bounds.center();
        let cell = self.logo_cell(bounds);
        let wave = Wave {
            center_x: center.x.as_f32(),
            center_y: center.y.as_f32(),
            cell,
            band: cell * 2.0,
            cols: (bounds.size.width.as_f32() / cell)
                .ceil()
                .approx_as::<usize>()
                .unwrap_or(0)
                .max(1),
            rows: (bounds.size.height.as_f32() / cell)
                .ceil()
                .approx_as::<usize>()
                .unwrap_or(0)
                .max(1),
            front,
            revealing,
            alpha,
            cover,
        };
        for row in 0..wave.rows {
            let dy = wave
                .cell
                .mul_add(row.approx_as::<f32>().unwrap_or_inf() + 0.5, -wave.center_y);
            wave.paint_row(row, dy, window);
        }
    }

    /// The blind: the `M` wave rises from the center and submerges everything.
    fn paint_bloom(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        Self::fill(bounds, self.background, window);
        let (origin_x, origin_y, cell_size) = self.logo_origin(bounds);
        self.paint_logo_cells(origin_x, origin_y, cell_size, window);
        let wave_end = self
            .logo_cell(bounds)
            .mul_add(2.0, Self::max_radius(bounds));
        let front = wave_end * ease_in(self.progress);
        let flash_alpha = self.progress.mul_add(0.7, 0.3);
        self.paint_wave(bounds, front, false, flash_alpha, self.primary, window);
    }

    /// The reveal: the same wave but the front is a hole that grows from the center, revealing the app.
    /// The covering state is the pixel state that preceded the dissolve:
    /// the bloom's uniform accent color when bloom ran,
    /// otherwise the settled glow scene (theme background + monolith)
    /// which is re-painted so it does not pop out of existence when the dissolve starts.
    fn paint_dissolve(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        let wave_end = self
            .logo_cell(bounds)
            .mul_add(2.0, Self::max_radius(bounds));
        let front = wave_end * ease_in(self.progress);
        let cover = if self.bloomed {
            self.primary
        } else {
            self.background
        };
        self.paint_wave(bounds, front, true, 1.0, cover, window);
        if !self.bloomed {
            self.paint_covered_monolith(bounds, front, window);
        }
    }

    /// The monolith cells of the settled glow scene,
    /// repainted only where the reveal still covers them
    /// (their centers lie beyond the hole's front).
    /// Cells already revealed are left for the app to show through.
    fn paint_covered_monolith(&self, bounds: Bounds<Pixels>, front: f32, window: &mut Window) {
        let (origin_x, origin_y, cell_size) = self.logo_origin(bounds);
        let center = bounds.center();
        let front_sq = front * front;
        for cell in &self.logo.cells {
            let x = cell_size.mul_add(0.5, cell.col.mul_add(cell_size, origin_x));
            let y = cell_size.mul_add(0.5, cell.row.mul_add(cell_size, origin_y));
            let dx = x - center.x.as_f32();
            let dy = y - center.y.as_f32();
            if dx.mul_add(dx, dy * dy) <= front_sq {
                continue;
            }
            let color = match cell.tier {
                Tier::Light | Tier::Inscription => self.primary,
                Tier::RightSide => self.tier_one,
                Tier::LeftSide => self.tier_two,
                Tier::Top => self.tier_inscription,
            };
            let x = cell.col.mul_add(cell_size, origin_x).round();
            let y = cell.row.mul_add(cell_size, origin_y).round();
            Self::fill(
                Bounds::new(point(px(x), px(y)), size(px(cell_size), px(cell_size))),
                color,
                window,
            );
        }
    }
}
