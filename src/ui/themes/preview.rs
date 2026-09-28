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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{
        fonts::FontCatalog,
        themes::{self, ThemeLibrary},
    };
    use gpui_kit::component::Root;
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{Focusable as _, TestAppContext, px, size};

    #[test]
    fn native_previews_follow_the_edited_appearance_without_changing_the_application() {
        let mut cx = TestAppContext::single();
        let (light, dark) = appearances(&cx);
        let global = cx.update(|cx| serde_json::to_value(cx.theme()).unwrap());
        let base_global = cx.update(|cx| gpui_kit::base::Theme::global(cx));
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
                assert_background(window, appearance.theme().background);
                window.click("markdown-title", cx);
                window.press("backspace", cx);
                assert_eq!(preview.read(cx).note.read(cx).value().to_string(), NOTE);
                window.click("preview-base", cx);
                assert!(window.find("base-view-Projects").visible());
                assert_background(window, appearance.theme().table_head);
                window.click("base-view-Projects", cx);
                assert!(window.find("base-view-Projects").visible());
                window.click("preview-graph", cx);
                window.render_frame(cx);
                assert!(window.find("graph-view").visible());
                assert_background(window, appearance.theme().background);
                window.click("preview-todo", cx);
                assert_background(window, appearance.theme().background);
                assert!(window.try_find("todo-add-btn").is_none());
                assert_eq!(window.find(("todo-check", 0_usize)).checked(), Some(false));
                window.click(("todo-check", 0_usize), cx);
                assert_eq!(window.find(("todo-check", 0_usize)).checked(), Some(false));
                preview
                    .read(cx)
                    .todo
                    .as_ref()
                    .unwrap()
                    .read(cx)
                    .focus_handle(cx)
                    .focus(window, cx);
                window.render_frame(cx);
                assert_ne!(appearance.theme().ring, cx.theme().ring);
                assert!(
                    window
                        .painted_quads()
                        .iter()
                        .any(|quad| { quad.border_color == appearance.theme().ring }),
                    "Todo search focus must paint the edited theme's ring"
                );
                // Editing the palette must repaint a retained, focused input too.
                let changed = if appearance.theme().is_dark() {
                    &light
                } else {
                    &dark
                };
                preview.update(cx, |preview, cx| {
                    preview.set_appearance(Some(changed.clone()), cx);
                });
                window.render_frame(cx);
                assert!(
                    window
                        .painted_quads()
                        .iter()
                        .any(|quad| { quad.border_color == changed.theme().ring })
                );
                window.click("preview-note", cx);
                assert_eq!(serde_json::to_value(cx.theme()).unwrap(), global);
                let restored_base = gpui_kit::base::Theme::global(cx);
                assert_eq!(restored_base.appearance, base_global.appearance);
                assert_eq!(restored_base.tokens, base_global.tokens);
                assert_eq!(
                    preview
                        .read(cx)
                        .appearance
                        .as_ref()
                        .unwrap()
                        .theme()
                        .background,
                    changed.theme().background
                );
                assert!(preview.read(cx).base.is_ok());
                assert!(preview.read(cx).graph.is_ok());
                assert!(preview.read(cx).todo.is_ok());
            })
            .unwrap();
        }
    }

    #[test]
    fn todo_preview_menus_follow_edits_while_open_and_remain_interactive() {
        let mut cx = TestAppContext::single();
        let (light, dark) = appearances(&cx);
        let global = cx.update(|cx| serde_json::to_value(cx.theme()).unwrap());
        let mut preview = None;
        let handle = cx.open_window(size(px(720.), px(600.)), |window, cx| {
            let view = cx.new(|cx| ThemePreview::new(window, cx));
            view.update(cx, |preview, cx| {
                preview.content = Content::Todo;
                preview.set_appearance(Some(light.clone()), cx);
            });
            preview = Some(view.clone());
            Root::new(view, window, cx)
        });
        let preview = preview.unwrap();
        for trigger in ["todo-preview-filter-trigger", "todo-preview-sort-trigger"] {
            cx.update_window(handle.into(), |_, window, cx| {
                preview.update(cx, |preview, cx| {
                    preview.set_appearance(Some(light.clone()), cx);
                });
                window.render_frame(cx);
                window.click(trigger, cx);
                assert_menu_background(window, light.theme().popover);
                preview.update(cx, |preview, cx| {
                    preview.set_appearance(Some(dark.clone()), cx);
                });
                window.render_frame(cx);
                assert_menu_background(window, dark.theme().popover);
                window.press("escape", cx);
            })
            .unwrap();
            cx.run_until_parked();
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                assert!(window.try_find("popup-menu").is_none());
                window.click(trigger, cx);
                assert_menu_background(window, dark.theme().popover);
                window.within("popup-menu").click(1_usize, cx);
            })
            .unwrap();
            cx.run_until_parked();
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                assert!(window.try_find("popup-menu").is_none());
                assert_eq!(serde_json::to_value(cx.theme()).unwrap(), global);
            })
            .unwrap();
        }
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            // The filter changed the visible rows, without completing or deleting tasks.
            assert!(window.find(("todo-check", 0_usize)).visible());
            assert!(window.try_find(("todo-check", 3_usize)).is_none());
        })
        .unwrap();
    }

    fn assert_menu_background(window: &Window, color: gpui_kit::Hsla) {
        let bounds = window
            .find("popup-menu")
            .bounds()
            .scale(window.scale_factor());
        assert!(
            window
                .painted_quads()
                .iter()
                .any(|quad| { quad.bounds == bounds && quad.background == color.into() }),
            "popup must paint the edited theme's background {color:?}"
        );
    }

    fn appearances(cx: &TestAppContext) -> (ResolvedAppearance, ResolvedAppearance) {
        cx.update(|cx| {
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
        })
    }

    fn assert_background(window: &Window, color: gpui_kit::Hsla) {
        assert!(
            window
                .painted_quads()
                .iter()
                .any(|quad| quad.background == color.into()),
            "preview must paint its edited background {color:?}"
        );
    }
}
