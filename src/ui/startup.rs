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

const RISE_S: f32 = 1.0;
const IGNITE_S: f32 = 2.0;
const GLOW_S: f32 = 2.0;
const BLOOM_S: f32 = 1.0;
const DISSOLVE_S: f32 = 1.0;
const TOTAL_S: f32 = RISE_S + IGNITE_S + GLOW_S + BLOOM_S + DISSOLVE_S;
const FRAME_DURATION: Duration = Duration::from_millis(16);

const GRID_MIN_CELL_PX: f32 = 6.0;

// Width of each of the `M`/`1`/`2` bands in the falling inscription wave,
// as a fraction of the wave period.
const INSCRIPTION_BAND: f32 = 0.07;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Rise,
    Ignite,
    Glow,
    Bloom,
    Dissolve,
    Done,
}

impl Phase {
    fn of(secs: f32) -> Self {
        if secs < RISE_S {
            Self::Rise
        } else if secs < RISE_S + IGNITE_S {
            Self::Ignite
        } else if secs < RISE_S + IGNITE_S + GLOW_S {
            Self::Glow
        } else if secs < RISE_S + IGNITE_S + GLOW_S + BLOOM_S {
            Self::Bloom
        } else if secs < TOTAL_S {
            Self::Dissolve
        } else {
            Self::Done
        }
    }

    fn progress(secs: f32, phase: Self) -> f32 {
        let (start, span) = match phase {
            Self::Rise => (0.0, RISE_S),
            Self::Ignite => (RISE_S, IGNITE_S),
            Self::Glow => (RISE_S + IGNITE_S, GLOW_S),
            Self::Bloom => (RISE_S + IGNITE_S + GLOW_S, BLOOM_S),
            Self::Dissolve => (RISE_S + IGNITE_S + GLOW_S + BLOOM_S, DISSOLVE_S),
            Self::Done => (TOTAL_S, 1.0),
        };
        ((secs - start) / span).clamp(0.0, 1.0)
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

pub struct StartupAnimation {
    started_at: Instant,
    finished: bool,
    needs_focus: bool,
    focus_handle: FocusHandle,
    logo: LogoGrid,
    phase: Phase,
    progress: f32,
    elapsed: f32,
}

impl StartupAnimation {
    pub fn new(cx: &Context<Self>) -> Self {
        Self {
            started_at: Instant::now(),
            finished: false,
            needs_focus: true,
            focus_handle: cx.focus_handle(),
            logo: parse_logo(super::monolith::LOGO_SRC),
            phase: Phase::Rise,
            progress: 0.0,
            elapsed: 0.0,
        }
    }

    fn advance(&mut self) -> bool {
        if self.finished {
            return true;
        }
        let secs = self.started_at.elapsed().as_secs_f32();
        let phase = Phase::of(secs);
        self.elapsed = secs;
        self.progress = Phase::progress(secs, phase);
        self.phase = phase;
        self.phase == Phase::Done
    }

    fn finish(&mut self, cx: &mut Context<Self>) {
        self.finished = true;
        self.phase = Phase::Done;
        self.progress = 1.0;
        self.elapsed = TOTAL_S;
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
    primary: Hsla,  // M color
    tier_one: Hsla, // 1 color
    tier_two: Hsla, // 2 color
    tier_inscription: Hsla,
    background: Hsla,
    logo: LogoGrid,
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

    /// The monolith border pulses through the three tiers `2 -> 1 -> M` cycling,
    /// so the outline reads as a glowing line.
    fn border_pulse(&self) -> Hsla {
        let third = 1.0 / 3.0;
        let p = (self.elapsed * 1.1) % 1.0;
        if p < third {
            lerp_hsla(self.tier_two, self.tier_one, p * 3.0)
        } else if p < 2.0 * third {
            lerp_hsla(self.tier_one, self.primary, (p - third) * 3.0)
        } else {
            lerp_hsla(self.primary, self.tier_two, third.mul_add(-2.0, p) * 3.0)
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
    /// cycling each glyph through `white -> M -> 1 -> 2`;
    /// during the glow the inscriptions turn to `M` uniformly with the border,
    /// and keep `M` through the flash.
    fn inscription_color(&self, cell: &Cell) -> Hsla {
        match self.phase {
            Phase::Ignite => {
                let wave = (self.progress * 2.0) % 1.0;
                let row_norm = cell.row / self.logo.height;
                let behind = (wave - row_norm + 1.0) % 1.0;
                if behind < INSCRIPTION_BAND {
                    self.primary
                } else if behind < INSCRIPTION_BAND * 2.0 {
                    self.tier_one
                } else if behind < INSCRIPTION_BAND * 3.0 {
                    self.tier_two
                } else {
                    self.tier_inscription
                }
            }
            Phase::Glow => self.uniform_settle(self.progress),
            Phase::Bloom => self.primary,
            _ => self.tier_inscription,
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

    /// Paint a ring of grid cells around the center for the blind/reveal wave.
    ///
    /// The front travels from the center outward.
    /// During the blind (`revealing` is false) cells are lit like this `2 -> 1 -> M`.
    /// During the reveal the front is the hole and the cells ahead of it go
    /// `2 -> 1 -> M` before being revealed to the app.
    fn paint_wave(
        &self,
        bounds: Bounds<Pixels>,
        front: f32,
        revealing: bool,
        alpha: f32,
        window: &mut Window,
    ) {
        let center = bounds.center();
        let center_x = center.x.as_f32();
        let center_y = center.y.as_f32();
        let cell = self.logo_cell(bounds);
        let cols = (bounds.size.width.as_f32() / cell)
            .ceil()
            .approx_as::<usize>()
            .unwrap_or(0)
            .max(1);
        let rows = (bounds.size.height.as_f32() / cell)
            .ceil()
            .approx_as::<usize>()
            .unwrap_or(0)
            .max(1);
        for row in 0..rows {
            for col in 0..cols {
                let x = col.approx_as::<f32>().unwrap_or_inf() * cell;
                let y = row.approx_as::<f32>().unwrap_or_inf() * cell;
                let dx = cell.mul_add(0.5, x) - center_x;
                let dy = cell.mul_add(0.5, y) - center_y;
                let distance = dx.mul_add(dx, dy * dy).sqrt();
                let offset = if revealing {
                    distance - front
                } else {
                    front - distance
                };
                if offset <= 0.0 {
                    continue;
                }
                let color = if offset <= cell {
                    self.tier_two
                } else if offset <= cell * 2.0 {
                    self.tier_one
                } else {
                    self.primary
                }
                .opacity(alpha);
                Self::fill(
                    Bounds::new(point(px(x), px(y)), size(px(cell), px(cell))),
                    color,
                    window,
                );
            }
        }
    }

    /// The blind: the `2 -> 1 -> M` wave rises from the center and submerges everythings,
    /// the flash starts translucent so the logo is seen being swallowed by the light,
    /// then turns opaque and covers it.
    fn paint_bloom(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        Self::fill(bounds, self.background, window);
        let (origin_x, origin_y, cell_size) = self.logo_origin(bounds);
        self.paint_logo_cells(origin_x, origin_y, cell_size, window);
        let wave_end = self
            .logo_cell(bounds)
            .mul_add(2.0, Self::max_radius(bounds));
        let front = wave_end * ease_in(self.progress);
        let flash_alpha = self.progress.mul_add(0.7, 0.3);
        self.paint_wave(bounds, front, false, flash_alpha, window);
    }

    /// The reveal: the same wave but the front is a hole that grows from the center,
    /// dimming the cells `M -> 1 -> 2` ahead of it and then revealing the app.
    fn paint_dissolve(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        let wave_end = self
            .logo_cell(bounds)
            .mul_add(2.0, Self::max_radius(bounds));
        let front = wave_end * ease_in(self.progress);
        self.paint_wave(bounds, front, true, 1.0, window);
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
