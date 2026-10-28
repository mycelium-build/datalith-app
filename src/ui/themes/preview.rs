//! Retained sample documents using Datalith's production renderers.
//! Appearance is passed to Datalith renderers and scoped to native components.
//! The surrounding editor keeps the application's theme.
mod theme_scope;
use std::{path::PathBuf, rc::Rc};
pub use theme_scope::themed;

use gpui_kit::component::{
    ActiveTheme as _, Selectable as _, Sizable as _, Theme,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::EditorState,
    v_flex,
};
use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement, Render, Styled, Window, div,
};

use crate::app::themes::ResolvedAppearance;
use crate::ui::{
    editors::todo_txt::{TodoTxtEditor, TodoTxtState},
    viewers::{base::BaseViewState, markdown::MarkdownViewer},
};

const NOTE: &str = "A quiet place to capture the details that matter. Follow the thread, then turn it into a plan.\n\n> Make room for the next idea.\n\nLink the draft to [Project Atlas](atlas.md).\n\n```rust\nlet plan = build(\"Atlas\", 3);\n```";
const TASKS: &str = "(A) 2026-09-25 Finalize the user journey +Atlas @design\n(B) 2026-09-25 Prepare the prototype +Atlas @desk\n2026-09-25 Read the team feedback +Atlas @reading\nx 2026-09-24 2026-09-22 Gather inspiration +Atlas\n";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Content {
    Note,
    Base,
    Graph,
    Todo,
}

pub(super) struct ThemePreview {
    appearance: Option<Rc<ResolvedAppearance>>,
    // Shared with the scoped renderers, so frames don't clone the theme.
    theme: Option<Rc<Theme>>,
    note: Entity<EditorState>,
    base: Result<Entity<BaseViewState>, String>,
    graph: Result<Entity<BaseViewState>, String>,
    todo: Result<Entity<TodoTxtState>, String>,
    content: Content,
}

impl ThemePreview {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            appearance: None,
            theme: None,
            note: cx.new(|cx| EditorState::new(window, cx).default_value(NOTE)),
            base: BaseViewState::preview(window, cx).map_err(|error| error.to_string()),
            graph: BaseViewState::graph_preview(window, cx).map_err(|error| error.to_string()),
            todo: TodoTxtEditor::preview_state(TASKS, window, cx)
                .map_err(|error| error.to_string()),
            content: Content::Note,
        }
    }

    pub(super) fn set_appearance(
        &mut self,
        appearance: Option<Rc<ResolvedAppearance>>,
        cx: &mut Context<Self>,
    ) {
        self.theme = appearance
            .as_ref()
            .map(|appearance| Rc::new(appearance.theme().clone()));
        // Base and Graph read the scoped global theme; Todo.txt also needs it
        // for its deferred filter and sort popups, which render outside the scope.
        if let Some(theme) = &self.theme
            && let Ok(todo) = &self.todo
        {
            todo.update(cx, |todo, cx| {
                todo.set_preview_appearance(theme.clone(), cx);
            });
        }
        self.appearance = appearance;
        cx.notify();
    }
}

impl Render for ThemePreview {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (Some(appearance), Some(theme)) = (&self.appearance, &self.theme) else {
            return div()
                .child("Theme preview is unavailable")
                .into_any_element();
        };
        let body = match self.content {
            Content::Note => {
                let viewer =
                    MarkdownViewer::new(self.note.clone(), PathBuf::from("Field notes.md"));
                viewer.render_document(
                    &self.note.read(cx).value(),
                    None,
                    appearance.theme(),
                    appearance.fonts(),
                    cx,
                )
            }
            Content::Base => entity_or_error(&self.base),
            Content::Graph => entity_or_error(&self.graph),
            Content::Todo => entity_or_error(&self.todo),
        };
        v_flex()
            .size_full()
            .min_h_0()
            .min_w_0()
            .child(
                h_flex()
                    .p_3()
                    .gap_2()
                    .flex_wrap()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("Preview"),
                    )
                    .children(
                        [
                            (Content::Note, "Note", "preview-note"),
                            (Content::Base, "Base", "preview-base"),
                            (Content::Graph, "Graph", "preview-graph"),
                            (Content::Todo, "Todo.txt", "preview-todo"),
                        ]
                        .into_iter()
                        .map(|(content, label, id)| {
                            Button::new(id)
                                .small()
                                .ghost()
                                .selected(self.content == content)
                                .label(label)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.content = content;
                                    cx.notify();
                                }))
                        }),
                    ),
            )
            .child(
                div()
                    .id("theme-preview-surface")
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .overflow_hidden()
                    .bg(appearance.theme().background)
                    .text_color(appearance.theme().foreground)
                    .font_family(appearance.fonts().interface().clone())
                    .child(themed(theme.clone(), body)),
            )
            .into_any_element()
    }
}

fn entity_or_error<T: Render>(entity: &Result<Entity<T>, String>) -> AnyElement {
    entity.as_ref().map_or_else(
        |error| div().p_3().child(error.clone()).into_any_element(),
        |entity| entity.clone().into_any_element(),
    )
}
