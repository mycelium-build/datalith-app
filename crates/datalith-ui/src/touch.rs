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
