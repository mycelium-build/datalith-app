//! Touch target sizes that follow the text size.
//!
//! A size is given in points at the default 16 pt text size. It scales with the
//! window's rem, so a larger text setting also enlarges rows and buttons, and it
//! never drops below the 44 pt minimum touch target.

use gpui_kit::{Styled, px, rems};

/// The smallest comfortable touch target, in points.
pub const MINIMUM: f32 = 44.;

/// Sizes a touch target from points at the default text size.
pub trait TouchSize: Styled + Sized {
    /// Sets the height, scaled with the text size, at least [`MINIMUM`].
    #[must_use]
    fn touch_h(self, points: f32) -> Self {
        self.h(rems(points / 16.)).min_h(px(points.max(MINIMUM)))
    }

    /// Sets the width, scaled with the text size, at least [`MINIMUM`].
    #[must_use]
    fn touch_w(self, points: f32) -> Self {
        self.w(rems(points / 16.)).min_w(px(points.max(MINIMUM)))
    }

    /// Sets a square size, scaled with the text size, at least [`MINIMUM`].
    #[must_use]
    fn touch_size(self, points: f32) -> Self {
        self.touch_h(points).touch_w(points)
    }
}

impl<E: Styled> TouchSize for E {}

/// Covers its parent and calls `action` when a long press starts inside it.
///
/// GPUI recognizes long presses but offers no element listener for them, so
/// this registers one while painting, scoped to the parent's visible bounds.
/// Give the parent `relative()`. The overlay has no hitbox, so taps and hover
/// still reach the parent.
pub fn long_press_area(
    action: impl Fn(gpui_kit::Point<gpui_kit::Pixels>, &mut gpui_kit::Window, &mut gpui_kit::App)
    + 'static,
) -> impl gpui_kit::IntoElement {
    use gpui_kit::{DispatchPhase, LongPressEvent, TouchPhase};
    gpui_kit::canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            // Clip to what is visible, so a row scrolled out of its list never
            // answers a press on whatever now sits at its old position.
            let bounds = bounds.intersect(&window.content_mask().bounds);
            window.on_mouse_event(move |event: &LongPressEvent, phase, window, cx| {
                if phase == DispatchPhase::Bubble
                    && event.phase == TouchPhase::Started
                    && bounds.contains(&event.start_position)
                    && !window.default_prevented()
                {
                    window.prevent_default();
                    cx.stop_propagation();
                    action(event.start_position, window, cx);
                }
            });
        },
    )
    .absolute()
    .size_full()
}
