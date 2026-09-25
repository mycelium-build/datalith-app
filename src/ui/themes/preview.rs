//! Retained sample documents using Datalith's production renderers.
//! Appearance is passed to each renderer; previews never mutate the global theme.
use std::{path::PathBuf, rc::Rc};

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

const NOTE: &str = "# Field notes\n\nA quiet place to capture the details that matter. Follow the thread, then turn it into a plan.\n\n> Make room for the next idea.\n\nLink the draft to [Project Atlas](atlas.md).\n\n```rust\nlet plan = build(\"Atlas\", 3);\n```";
const TASKS: &str = "(A) 2026-09-25 Finalize the user journey +Atlas @design\n(B) 2026-09-25 Prepare the prototype +Atlas @desk\n2026-09-25 Read the team feedback +Atlas @reading\nx 2026-09-24 2026-09-22 Gather inspiration +Atlas\n";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Content {
    Note,
    Base,
    Todo,
}

pub(super) struct ThemePreview {
    appearance: Option<ResolvedAppearance>,
    note: Entity<EditorState>,
    base: Result<Entity<BaseViewState>, String>,
    todo: Result<Entity<TodoTxtState>, String>,
    content: Content,
}

impl ThemePreview {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            appearance: None,
            note: cx.new(|cx| EditorState::new(window, cx).default_value(NOTE)),
            base: BaseViewState::preview(window, cx).map_err(|error| error.to_string()),
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
                    .child(div().flex_1().child("Preview"))
                    .children(
                        [
                            (Content::Note, "Note", "preview-note"),
                            (Content::Base, "Base", "preview-base"),
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
                    .child(body),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{
        fonts::FontCatalog,
        themes::{self, ThemeLibrary},
    };
    use gpui_kit::component::Root;
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{TestAppContext, px, size};

    #[test]
    fn native_previews_follow_the_edited_appearance_without_changing_the_application() {
        let mut cx = TestAppContext::single();
        let (light, dark) = cx.update(|cx| {
            gpui_kit::init(cx);
            FontCatalog::init(cx);
            themes::load_embedded_themes(cx);
            ThemeLibrary::init(cx);
            let library = cx.global::<ThemeLibrary>();
            let variants = library.family_named("Catppuccin").unwrap().variants();
            let resolve = |dark| {
                library
                    .resolved(
                        variants
                            .iter()
                            .find(|variant| variant.mode().is_dark() == dark)
                            .unwrap()
                            .id(),
                        cx.global::<FontCatalog>(),
                    )
                    .unwrap()
            };
            (resolve(false), resolve(true))
        });
        let global = cx.update(|cx| cx.theme().background);
        let mut preview = None;
        let handle = cx.open_window(size(px(720.), px(600.)), |window, cx| {
            let view = cx.new(|cx| ThemePreview::new(window, cx));
            preview = Some(view.clone());
            Root::new(view, window, cx)
        });
        let preview = preview.unwrap();
        for appearance in [&light, &dark] {
            cx.update_window(handle.into(), |_, window, cx| {
                preview.update(cx, |preview, cx| {
                    preview.set_appearance(Some(appearance.clone()), cx);
                });
                window.render_frame(cx);
                window.click("preview-base", cx);
                assert!(window.find("base-view-Projects").visible());
                window.click("base-view-Projects", cx);
                assert!(window.find("base-view-Projects").visible());
                window.click("preview-todo", cx);
                assert!(window.find("todo-add-btn").visible());
                window.click("preview-note", cx);
                assert_eq!(cx.theme().background, global);
                assert_eq!(
                    preview
                        .read(cx)
                        .appearance
                        .as_ref()
                        .unwrap()
                        .theme()
                        .background,
                    appearance.theme().background
                );
                assert!(preview.read(cx).base.is_ok());
                assert!(preview.read(cx).todo.is_ok());
            })
            .unwrap();
        }
    }
}
