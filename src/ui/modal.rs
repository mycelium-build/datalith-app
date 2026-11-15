use gpui_kit::base::actions::Cancel;
use gpui_kit::component::ActiveTheme;
use gpui_kit::{
    App, Context, Div, ElementId, InteractiveElement, KeyBinding, Stateful,
    StatefulInteractiveElement, Styled, div, px,
};

use super::DatalithView;

const MODAL_CONTEXT: &str = "Modal";

/// Binds Escape to `Cancel` in a modal panel.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", Cancel, Some(MODAL_CONTEXT))]);
}

/// Full-window dimmed layer behind a modal; clicking it calls `close`.
pub fn backdrop(
    id: impl Into<ElementId>,
    cx: &Context<DatalithView>,
    close: fn(&mut DatalithView),
) -> Stateful<Div> {
    div()
        .absolute()
        .inset_0()
        .bg(gpui_kit::black().opacity(0.3))
        .flex()
        .items_center()
        .justify_center()
        .id(id)
        // Keep wheel and pointer events on the modal, including at its scroll boundaries.
        .occlude()
        .on_click(cx.listener(move |view: &mut DatalithView, _, _, cx| {
            close(view);
            cx.notify();
        }))
}

/// Themed modal container; clicks inside it do not reach the backdrop.
/// Handle `Cancel` on it to close it with Escape.
pub fn panel(id: impl Into<ElementId>, cx: &Context<DatalithView>) -> Stateful<Div> {
    div()
        .key_context(MODAL_CONTEXT)
        .bg(cx.theme().background)
        .border(px(1.))
        .border_color(cx.theme().border)
        .rounded_md()
        .shadow_lg()
        .id(id)
        .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()))
}
