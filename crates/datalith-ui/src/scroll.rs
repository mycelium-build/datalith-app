use gpui_kit::base::ScrollbarHandle;
use gpui_kit::base::actions::{
    SelectDown, SelectFirst, SelectLast, SelectPageDown, SelectPageUp, SelectUp,
};
use gpui_kit::{Action, InteractiveElement, ListState, ScrollHandle, Window, point, px};
#[derive(Clone, Copy)]
enum Step {
    LineDown,
    LineUp,
    PageDown,
    PageUp,
    Top,
    Bottom,
}

pub trait KeyScrollHandle: ScrollbarHandle + Clone {
    fn scroll_to_bottom(&self);
}

impl KeyScrollHandle for ScrollHandle {
    fn scroll_to_bottom(&self) {
        Self::scroll_to_bottom(self);
    }
}

impl KeyScrollHandle for ListState {
    fn scroll_to_bottom(&self) {
        // Offscreen rows may have estimated heights. Anchor to the last item.
        self.scroll_to_end();
    }
}

/// Scrolls `handle` with the keys while `element`, or a child, has focus.
pub fn scroll_with_keys<E: InteractiveElement>(element: E, handle: &impl KeyScrollHandle) -> E {
    let element = element.key_context(crate::contexts::SCROLL_CONTEXT);
    let element = on_step::<SelectDown, E>(element, handle, Step::LineDown);
    let element = on_step::<SelectUp, E>(element, handle, Step::LineUp);
    let element = on_step::<SelectPageDown, E>(element, handle, Step::PageDown);
    let element = on_step::<SelectPageUp, E>(element, handle, Step::PageUp);
    let element = on_step::<SelectFirst, E>(element, handle, Step::Top);
    on_step::<SelectLast, E>(element, handle, Step::Bottom)
}

fn on_step<A: Action, E: InteractiveElement>(
    element: E,
    handle: &impl KeyScrollHandle,
    step: Step,
) -> E {
    let handle = handle.clone();
    element.on_action(move |_: &A, window, _| scroll(&handle, step, window))
}

#[allow(
    clippy::arithmetic_side_effects,
    reason = "floating-point pixel arithmetic; scroll offsets are clamped to bounds"
)]
fn scroll(handle: &impl KeyScrollHandle, step: Step, window: &mut Window) {
    let current = handle.offset();
    let max = (handle.content_size().height - handle.viewport_bounds().size.height).max(px(0.));
    let line = window.rem_size() * 2.0;
    let page = handle.viewport_bounds().size.height * 0.85;
    let next = match step {
        Step::LineDown => current.y - line,
        Step::LineUp => current.y + line,
        Step::PageDown => current.y - page,
        Step::PageUp => current.y + page,
        Step::Top => px(0.),
        Step::Bottom => {
            handle.scroll_to_bottom();
            window.refresh();
            return;
        }
    };
    handle.set_offset(point(current.x, next.clamp(-max, px(0.))));
    window.refresh();
}
