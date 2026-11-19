use std::path::{Path, PathBuf};

use gpui_kit::component::input::{Editor, EditorState, Enter, Escape, MoveDown, MoveUp};
use gpui_kit::{
    Action, AnyElement, App, AppContext, Context, Div, Entity, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, MouseButton, ParentElement, Styled, Window, div, px,
};

use crate::LINE_HEIGHT;
use crate::actions::{Bold, InsertWikiLink, Italic};
use datalith_vault::VaultCatalog;

mod wikilink;

use wikilink::WikiLinkAutocomplete;

pub struct MarkdownEditor {
    input: Entity<EditorState>,
    source: PathBuf,
    autocomplete: Option<Entity<WikiLinkAutocomplete>>,
    read_only: bool,
}

impl MarkdownEditor {
    pub fn new_state(path: &Path, window: &mut Window, cx: &mut App) -> Entity<EditorState> {
        let content = datalith_vault::source::read_to_string(path).unwrap_or_default();
        cx.new(|cx| {
            EditorState::new(window, cx)
                .language("markdown")
                .line_number(false)
                .folding(false)
                .default_value(content)
        })
    }

    #[must_use]
    pub fn new(input: Entity<EditorState>, source: &Path, read_only: bool) -> Self {
        Self {
            input,
            source: source.to_path_buf(),
            autocomplete: None,
            read_only,
        }
    }

    pub fn set_vault_catalog(&mut self, catalog: VaultCatalog, cx: &mut App) {
        if self.read_only {
            return;
        }

        if let Some(autocomplete) = &self.autocomplete {
            autocomplete.update(cx, |autocomplete, cx| autocomplete.set_catalog(catalog, cx));
        } else {
            let input = self.input.clone();
            let source = self.source.clone();
            self.autocomplete =
                Some(cx.new(|cx| WikiLinkAutocomplete::new(input, source, catalog, cx)));
        }
    }

    #[must_use]
    pub const fn input(&self) -> &Entity<EditorState> {
        &self.input
    }

    pub fn render(&self, cx: &mut App) -> AnyElement {
        let base_font_size = crate::fonts::editor_font_size(cx);
        let line_height = LINE_HEIGHT;

        let mut container = div().relative().size_full();
        if !self.read_only {
            container = capture_menu_key::<Enter>(
                container,
                self.autocomplete.clone(),
                WikiLinkAutocomplete::handle_enter,
            );
            container = capture_menu_key::<MoveUp>(
                container,
                self.autocomplete.clone(),
                WikiLinkAutocomplete::handle_up,
            );
            container = capture_menu_key::<MoveDown>(
                container,
                self.autocomplete.clone(),
                WikiLinkAutocomplete::handle_down,
            );
            container = capture_menu_key::<Escape>(
                container,
                self.autocomplete.clone(),
                WikiLinkAutocomplete::handle_escape,
            );

            container = wrap_on::<Bold>(container, &self.input, "**", "**");
            container = wrap_on::<Italic>(container, &self.input, "*", "*");
            container = wrap_on::<InsertWikiLink>(container, &self.input, "[[", "]]");

            let autocomplete = self.autocomplete.clone();
            container = container.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                if let Some(autocomplete) = autocomplete.as_ref() {
                    autocomplete.update(cx, |menu, cx| {
                        menu.handle_mouse_down(window, cx);
                    });
                }
            });
        }

        container
            .child(
                Editor::new(&self.input)
                    .h_full()
                    .appearance(false)
                    .readonly(self.read_only)
                    .text_size(px(base_font_size))
                    .line_height(px(base_font_size * line_height)),
            )
            .children(self.autocomplete.clone())
            .into_any_element()
    }

    pub(crate) fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}

/// Wraps the selection in `open` and `close` on `A`, or unwraps it when it
/// is already wrapped, then keeps the inner text selected.
fn wrap_on<A: Action + 'static>(
    container: Div,
    input: &Entity<EditorState>,
    open: &'static str,
    close: &'static str,
) -> Div {
    let input = input.clone();
    container.on_action(move |_: &A, window, cx| {
        input.update(cx, |state, cx| {
            let selection = state.selected_range();
            let start = selection.start;
            let selected = state.selected_text().to_string();
            let (replacement, inner) = toggle_wrap(&selected, open, close);
            // replace() writes the same text at every cursor. This replacement
            // was computed from the active selection only.
            state.set_selected_range(selection, cx);
            state.replace(replacement, window, cx);
            let inner_start = start.saturating_add(inner.start);
            let inner_end = start.saturating_add(inner.end);
            state.set_selected_range(inner_start..inner_end, cx);
        });
    })
}

/// The text replacing `selected`, and the range of its inner text.
fn toggle_wrap(selected: &str, open: &str, close: &str) -> (String, std::ops::Range<usize>) {
    let unwrapped = selected
        .strip_prefix(open)
        .and_then(|rest| rest.strip_suffix(close))
        .filter(|_| selected.len() >= open.len().saturating_add(close.len()));
    unwrapped.map_or_else(
        || {
            let inner_end = open.len().saturating_add(selected.len());
            (format!("{open}{selected}{close}"), open.len()..inner_end)
        },
        |inner| (inner.to_owned(), 0..inner.len()),
    )
}

/// Routes wiki-link popup keys before the editor handles them itself.
fn capture_menu_key<A: Action + 'static>(
    container: Div,
    autocomplete: Option<Entity<WikiLinkAutocomplete>>,
    handler: fn(&mut WikiLinkAutocomplete, &Window, &mut Context<WikiLinkAutocomplete>) -> bool,
) -> Div {
    container.capture_action(move |_action: &A, window, cx| {
        let Some(autocomplete) = autocomplete.as_ref() else {
            return;
        };
        let consumed = autocomplete.update(cx, |menu, cx| handler(menu, window, cx));
        if consumed {
            cx.stop_propagation();
        }
    })
}

#[cfg(test)]
mod tests {
    use super::toggle_wrap;

    #[test]
    fn toggle_wrap_wraps_then_unwraps() {
        assert_eq!(
            toggle_wrap("word", "**", "**"),
            ("**word**".to_owned(), 2..6)
        );
        assert_eq!(
            toggle_wrap("**word**", "**", "**"),
            ("word".to_owned(), 0..4)
        );
        assert_eq!(toggle_wrap("", "[[", "]]"), ("[[]]".to_owned(), 2..2));
        assert_eq!(toggle_wrap("*", "*", "*"), ("***".to_owned(), 1..2));
    }
}
