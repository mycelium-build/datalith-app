use std::path::{Path, PathBuf};

use gpui_kit::component::input::{Editor, EditorState, Enter, Escape, MoveDown, MoveUp};
use gpui_kit::{
    Action, AnyElement, App, AppContext, Context, Div, Entity, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, MouseButton, ParentElement, Styled, Window, div, px,
};

use crate::ui::{BASE_FONT_SIZE, LINE_HEIGHT};
use crate::vault::VaultCatalog;

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
        let content = crate::vault::source::read_to_string(path).unwrap_or_default();
        cx.new(|cx| {
            EditorState::new(window, cx)
                .language("markdown")
                .line_number(false)
                .folding(false)
                .default_value(content)
        })
    }

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

    pub const fn input(&self) -> &Entity<EditorState> {
        &self.input
    }

    pub fn render(&self, _cx: &mut App) -> AnyElement {
        let base_font_size = BASE_FONT_SIZE;
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
