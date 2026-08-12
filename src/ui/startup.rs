use std::time::{Duration, Instant};

use conv::{ConvUtil, UnwrapOrInf};
use gpui::{
    App, BorderStyle, Bounds, Context, Corners, Edges, Element, ElementId, FocusHandle, Hsla,
    InteractiveElement, IntoElement, KeyDownEvent, LayoutId, Length, ParentElement, Pixels, Render,
    Size, StatefulInteractiveElement, Style, Styled, Window, div, point, px, quad, size,
    solid_background, transparent_black,
};
use gpui_component::ActiveTheme;

use super::DatalithView;
use super::monolith::{
    Cell, LEFT_SIDE_WHITEN, LogoGrid, RIGHT_SIDE_WHITEN, Tier, parse_logo, whiten,
};

const FRAME_DURATION: Duration = Duration::from_millis(32);
const WAVES_PER_IGNITE: f32 = 2.0;

const GRID_MIN_CELL_PX: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Rise,
    Ignite,
    Glow,
    Bloom,
    Dissolve,
    Done,
}

/// The duration, in seconds, of each animation phase.
#[derive(Clone, Copy, Debug)]
struct StartupTiming {
    rise: f32,
    ignite: f32,
    glow: f32,
    bloom: f32,
    dissolve: f32,
}

impl StartupTiming {
    const FIRST: Self = Self {
        rise: 0.5,
        ignite: 2.0,
        glow: 1.5,
        bloom: 0.75,
        dissolve: 0.75,
    };

    const STANDARD: Self = Self {
        rise: 0.0,
        ignite: 0.0,
        glow: 0.5,
        bloom: 0.0,
        dissolve: 0.75,
    };

    const fn total(self) -> f32 {
        self.rise + self.ignite + self.glow + self.bloom + self.dissolve
    }

    fn phase_of(self, secs: f32) -> Phase {
        if secs < self.rise {
            Phase::Rise
        } else if secs < self.rise + self.ignite {
            Phase::Ignite
        } else if secs < self.rise + self.ignite + self.glow {
            Phase::Glow
        } else if secs < self.rise + self.ignite + self.glow + self.bloom {
            Phase::Bloom
        } else if secs < self.total() {
            Phase::Dissolve
        } else {
            Phase::Done
        }
    }

    fn phase_progress(self, secs: f32, phase: Phase) -> f32 {
        let (start, span) = match phase {
            Phase::Rise => (0.0, self.rise),
            Phase::Ignite => (self.rise, self.ignite),
            Phase::Glow => (self.rise + self.ignite, self.glow),
            Phase::Bloom => (self.rise + self.ignite + self.glow, self.bloom),
            Phase::Dissolve => (
                self.rise + self.ignite + self.glow + self.bloom,
                self.dissolve,
            ),
            Phase::Done => (self.total(), 1.0),
        };
        ((secs - start) / span).clamp(0.0, 1.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupType {
    First,
    Standard,
}

impl StartupType {
    const fn timing(self) -> StartupTiming {
        match self {
            Self::First => StartupTiming::FIRST,
            Self::Standard => StartupTiming::STANDARD,
        }
    }
}

fn lerp_hsla(a: Hsla, b: Hsla, t: f32) -> Hsla {
    Hsla {
        h: (b.h - a.h).mul_add(t, a.h),
        s: (b.s - a.s).mul_add(t, a.s),
        l: (b.l - a.l).mul_add(t, a.l),
        a: (b.a - a.a).mul_add(t, a.a),
    }
}

fn ease_in(t: f32) -> f32 {
    t * t * t
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

fn ease_in_out(t: f32) -> f32 {
    t * t * 2.0f32.mul_add(-t, 3.0)
}

pub struct StartupAnimation {
    started_at: Instant,
    finished: bool,
    needs_focus: bool,
    focus_handle: FocusHandle,
    logo: LogoGrid,
    phase: Phase,
    progress: f32,
    elapsed: f32,
    timing: StartupTiming,
}

impl StartupAnimation {
    pub fn new(kind: StartupType, cx: &Context<Self>) -> Self {
        let timing = kind.timing();
        Self {
            started_at: Instant::now(),
            finished: false,
            needs_focus: true,
            focus_handle: cx.focus_handle(),
            logo: parse_logo(super::monolith::LOGO_SRC),
            phase: timing.phase_of(0.0),
            progress: 0.0,
            elapsed: 0.0,
            timing,
        }
    }

    fn advance(&mut self) -> bool {
        if self.finished {
            return true;
        }
        let secs = self.started_at.elapsed().as_secs_f32();
        let phase = self.timing.phase_of(secs);
        self.elapsed = secs;
        self.progress = self.timing.phase_progress(secs, phase);
        self.phase = phase;
        self.phase == Phase::Done
    }

    fn finish(&mut self, cx: &mut Context<Self>) {
        self.finished = true;
        self.phase = Phase::Done;
        self.progress = 1.0;
        self.elapsed = self.timing.total();
        cx.notify();
    }
}

impl Render for StartupAnimation {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.needs_focus {
            self.needs_focus = false;
            self.focus_handle.focus(window, cx);
        }
        let theme = cx.theme();
        let primary = theme.primary;
        let tier_one = whiten(primary, RIGHT_SIDE_WHITEN);
        let tier_two = whiten(primary, LEFT_SIDE_WHITEN);

        let element = MonolithElement {
            phase: self.phase,
            progress: self.progress,
            elapsed: self.elapsed,
            bloomed: self.timing.bloom > 0.0,
            primary,
            tier_one,
            tier_two,
            tier_inscription: gpui::white(),
            background: theme.background,
            logo: self.logo.clone(),
        };

        div()
            .absolute()
            .inset_0()
            .id("startup-overlay")
            .track_focus(&self.focus_handle)
            .on_click(cx.listener(|this, _, _, cx| this.finish(cx)))
            .on_key_down(cx.listener(|this, _: &KeyDownEvent, _, cx| this.finish(cx)))
            .child(element)
    }
}

struct MonolithElement {
    phase: Phase,
    progress: f32,
    elapsed: f32,
    bloomed: bool,
    primary: Hsla,  // M color
    tier_one: Hsla, // 1 color
    tier_two: Hsla, // 2 color
    tier_inscription: Hsla,
    background: Hsla,
    logo: LogoGrid,
}

/// The geometry and timing of one wave frame:
/// concentric rings around the window center whose front has travelled `front` pixels from the center.
struct Wave {
    center_x: f32,
    center_y: f32,
    cell: f32,
    band: f32,
    cols: usize,
    rows: usize,
    front: f32,
    revealing: bool,
    alpha: f32,
    cover: Hsla,
}

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
        _id: Option<&gpui::GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
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
        _id: Option<&gpui::GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _id: Option<&gpui::GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
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
    fn fill(bounds: Bounds<Pixels>, color: Hsla, window: &mut Window) {
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

impl Wave {
    fn paint_row(&self, row: usize, dy: f32, window: &mut Window) {
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
    fn paint_blind_row(&self, row: usize, dy: f32, window: &mut Window) {
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
    fn paint_reveal_row(&self, row: usize, dy: f32, window: &mut Window) {
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

    fn paint_band_cells(
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

    fn paint_row_strip(
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

impl DatalithView {
    pub(crate) fn step_startup(
        &mut self,
        startup: &gpui::Entity<StartupAnimation>,
        cx: &mut Context<Self>,
    ) -> bool {
        let done = startup.update(cx, |this, _| this.advance());
        if done {
            self.startup = None;
            if self.tabs.is_empty() {
                self.focus_sidebar_requested = true;
            } else {
                self.focus_editor_requested = true;
            }
        }
        cx.notify();
        done
    }

    pub(crate) fn spawn_startup_driver(&mut self, cx: &Context<Self>) {
        let Some(startup) = self.startup.clone() else {
            return;
        };
        self.startup_driver = cx.spawn(async move |this, cx| {
            loop {
                let done = this
                    .update(cx, |view, cx| view.step_startup(&startup, cx))
                    .unwrap_or(true);
                if done {
                    break;
                }
                cx.background_executor().timer(FRAME_DURATION).await;
            }
        });
    }
}
