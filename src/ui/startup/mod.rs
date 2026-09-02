use std::time::{Duration, Instant};

use gpui::{
    Context, FocusHandle, Hsla, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    Render, StatefulInteractiveElement, Styled, Window, div,
};
use gpui_component::ActiveTheme;

mod paint;
mod wave;

use paint::MonolithElement;

use super::DatalithView;
use crate::ui::monolith::{LEFT_SIDE_WHITEN, LogoGrid, RIGHT_SIDE_WHITEN, parse_logo, whiten};

const FRAME_DURATION: Duration = Duration::from_millis(32);

pub(super) const GRID_MIN_CELL_PX: f32 = 6.0;
pub(super) const WAVES_PER_IGNITE: f32 = 2.0;

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

pub(super) fn lerp_hsla(a: Hsla, b: Hsla, t: f32) -> Hsla {
    Hsla {
        h: (b.h - a.h).mul_add(t, a.h),
        s: (b.s - a.s).mul_add(t, a.s),
        l: (b.l - a.l).mul_add(t, a.l),
        a: (b.a - a.a).mul_add(t, a.a),
    }
}

pub(super) fn ease_in(t: f32) -> f32 {
    t * t * t
}

pub(super) fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

pub(super) fn ease_in_out(t: f32) -> f32 {
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
            logo: parse_logo(crate::ui::monolith::LOGO_SRC),
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
