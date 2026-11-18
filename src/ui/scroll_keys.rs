//! Fixed scrolling keys for views that scroll as a whole, like a note being
//! read. They aren't customizable.

use gpui_kit::base::actions::{
    SelectDown, SelectFirst, SelectLast, SelectPageDown, SelectPageUp, SelectUp,
};
use gpui_kit::{Action, App, InteractiveElement, KeyBinding, ScrollHandle, Window, point, px};

const CONTEXT: &str = "ScrollView";

pub fn bind_keys(cx: &mut App) {
    let context = Some(CONTEXT);
    let typing_excluded = Some("ScrollView && !Input");
    cx.bind_keys([
        KeyBinding::new("down", SelectDown, context),
        KeyBinding::new("up", SelectUp, context),
        KeyBinding::new("pagedown", SelectPageDown, context),
        // A text field inside doesn't bind Space, so it would lose it.
        KeyBinding::new("space", SelectPageDown, typing_excluded),
        KeyBinding::new("pageup", SelectPageUp, context),
        KeyBinding::new("shift-space", SelectPageUp, typing_excluded),
        KeyBinding::new("home", SelectFirst, context),
        KeyBinding::new("end", SelectLast, context),
    ]);
}

#[derive(Clone, Copy)]
enum Step {
    LineDown,
    LineUp,
    PageDown,
    PageUp,
    Top,
    Bottom,
}

/// Scrolls `handle` with the keys while `element`, or a child, has focus.
pub fn scroll_with_keys<E: InteractiveElement>(element: E, handle: &ScrollHandle) -> E {
    let element = element.key_context(CONTEXT);
    let element = on_step::<SelectDown, E>(element, handle, Step::LineDown);
    let element = on_step::<SelectUp, E>(element, handle, Step::LineUp);
    let element = on_step::<SelectPageDown, E>(element, handle, Step::PageDown);
    let element = on_step::<SelectPageUp, E>(element, handle, Step::PageUp);
    let element = on_step::<SelectFirst, E>(element, handle, Step::Top);
    on_step::<SelectLast, E>(element, handle, Step::Bottom)
}

fn on_step<A: Action, E: InteractiveElement>(element: E, handle: &ScrollHandle, step: Step) -> E {
    let handle = handle.clone();
    element.on_action(move |_: &A, window, _| scroll(&handle, step, window))
}

#[allow(
    clippy::arithmetic_side_effects,
    reason = "floating-point pixel arithmetic; scroll offsets are clamped to bounds"
)]
fn scroll(handle: &ScrollHandle, step: Step, window: &mut Window) {
    let current = handle.offset();
    let max = handle.max_offset().y;
    let line = window.rem_size() * 2.0;
    let page = handle.bounds().size.height * 0.85;
    let next = match step {
        Step::LineDown => current.y - line,
        Step::LineUp => current.y + line,
        Step::PageDown => current.y - page,
        Step::PageUp => current.y + page,
        Step::Top => px(0.),
        Step::Bottom => -max,
    };
    handle.set_offset(point(current.x, next.clamp(-max, px(0.))));
    window.refresh();
}
