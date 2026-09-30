//! Retained sample documents using Datalith's production renderers.
//! Appearance is passed to Datalith renderers and scoped to native components.
//! The surrounding editor keeps the application's theme.
mod theme_scope;
use std::{path::PathBuf, rc::Rc};
pub use theme_scope::themed;

use gpui_kit::component::{
    ActiveTheme as _, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::EditorState,
    v_flex,
};
use gpui_kit::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement, Render,
    Styled, Window, div,
};

use crate::app::{settings::FontRole, themes::ResolvedAppearance};
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
    appearance: Option<ResolvedAppearance>,
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
        appearance: Option<ResolvedAppearance>,
        cx: &mut Context<Self>,
    ) {
        if let Some(appearance) = &appearance {
            let theme = Rc::new(appearance.theme().clone());
            if let Ok(base) = &self.base {
                base.update(cx, |base, cx| {
                    base.set_preview_appearance(theme.clone(), cx);
                });
            }
            if let Ok(graph) = &self.graph {
                graph.update(cx, |graph, cx| {
                    graph.set_preview_appearance(theme.clone(), cx);
                });
            }
            if let Ok(todo) = &self.todo {
                todo.update(cx, |todo, cx| todo.set_preview_appearance(theme, cx));
            }
        }
        self.appearance = appearance;
        cx.notify();
    }
}

impl Render for ThemePreview {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(appearance) = &self.appearance else {
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
                    &[
                        appearance.font(FontRole::Reading).clone(),
                        appearance.font(FontRole::Headings).clone(),
                        appearance.font(FontRole::Code).clone(),
                    ],
                    cx,
                )
            }
            Content::Base => self.base.as_ref().map_or_else(
                |error| div().p_3().child(error.clone()).into_any_element(),
                |base| base.clone().into_any_element(),
            ),
            Content::Graph => self.graph.as_ref().map_or_else(
                |error| div().p_3().child(error.clone()).into_any_element(),
                |graph| graph.clone().into_any_element(),
            ),
            Content::Todo => self.todo.as_ref().map_or_else(
                |error| div().p_3().child(error.clone()).into_any_element(),
                |todo| todo.clone().into_any_element(),
            ),
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
                    .font_family(appearance.font(FontRole::Interface).clone())
                    .child(themed(Rc::new(appearance.theme().clone()), body)),
            )
            .into_any_element()
    }
}
