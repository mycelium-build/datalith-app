//! Extend the shared GPUI text handler with the document length `UIKit` requests.
//! The component retains ownership of selection, composition, editing and geometry.
use gpui_kit::base::input::{InputBaseState, InputModeKind};
use gpui_kit::{
    App, Bounds, ClipboardItem, ElementInputHandler, Entity, Focusable, InputHandler, Pixels,
    Point, Styled, TextInputConfiguration, UTF16Selection, Window, canvas,
};
use std::ops::Range;

pub fn registration<M: InputModeKind>(
    input: &Entity<InputBaseState<M>>,
) -> impl gpui_kit::IntoElement {
    let input = input.clone();
    canvas(
        |_, _, _| (),
        move |_, (), window, cx| {
            let state = input.read(cx);
            if !state.is_editable() {
                return;
            }
            let focus = state.focus_handle(cx);
            if focus.is_focused(window) {
                let inner = ElementInputHandler::new(state.input_bounds(), input.clone());
                window.handle_input(
                    &focus,
                    TextHandler {
                        inner,
                        input: input.clone(),
                    },
                    cx,
                );
            }
        },
    )
    .absolute()
}

struct TextHandler<M: InputModeKind> {
    inner: ElementInputHandler<InputBaseState<M>>,
    input: Entity<InputBaseState<M>>,
}

impl<M: InputModeKind> InputHandler for TextHandler<M> {
    fn text_length_utf16(&mut self, _: &mut Window, cx: &mut App) -> Option<usize> {
        Some(self.input.read(cx).text().len_utf16())
    }

    fn selected_text_range(
        &mut self,
        ignore_disabled_input: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<UTF16Selection> {
        self.inner
            .selected_text_range(ignore_disabled_input, window, cx)
    }

    fn marked_text_range(&mut self, window: &mut Window, cx: &mut App) -> Option<Range<usize>> {
        self.inner.marked_text_range(window, cx)
    }

    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<String> {
        self.inner
            .text_for_range(range_utf16, adjusted_range, window, cx)
    }

    fn replace_text_in_range(
        &mut self,
        replacement_range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.inner
            .replace_text_in_range(replacement_range, text, window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.inner.replace_and_mark_text_in_range(
            range_utf16,
            new_text,
            new_selected_range,
            window,
            cx,
        );
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut App) {
        self.inner.unmark_text(window, cx);
    }

    fn paste(&mut self, item: ClipboardItem, window: &mut Window, cx: &mut App) {
        self.inner.paste(item, window, cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Bounds<Pixels>> {
        self.inner.bounds_for_range(range_utf16, window, cx)
    }

    fn selection_bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<Bounds<Pixels>> {
        self.inner
            .selection_bounds_for_range(range_utf16, window, cx)
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<usize> {
        self.inner.character_index_for_point(point, window, cx)
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.inner.set_selected_text_range(range_utf16, window, cx);
    }

    fn element_bounds(&mut self, window: &mut Window, cx: &mut App) -> Option<Bounds<Pixels>> {
        self.inner.element_bounds(window, cx)
    }

    fn accepts_text_input(&mut self, window: &mut Window, cx: &mut App) -> bool {
        self.inner.accepts_text_input(window, cx)
    }

    fn prefers_ime_for_printable_keys(&mut self, window: &mut Window, cx: &mut App) -> bool {
        self.inner.prefers_ime_for_printable_keys(window, cx)
    }

    fn text_input_configuration(
        &mut self,
        window: &mut Window,
        cx: &mut App,
    ) -> TextInputConfiguration {
        self.inner.text_input_configuration(window, cx)
    }
}
