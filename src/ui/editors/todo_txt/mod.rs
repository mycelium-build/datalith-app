mod constants;
mod priority;
mod render;
mod state;
mod task_row;

use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext, Context, Entity, FocusHandle, Focusable, Hsla, IntoElement,
    ParentElement, Size, Styled, Subscription, Window, div, px, rgb,
};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::select::{SelectEvent, SelectState};
use gpui_component::{IndexPath, VirtualListScrollHandle};

use crate::document::handler::{FileHandler, ReloadOutcome};
use crate::document::todo_txt::{FilterKind, SortKind, TodoTxtWorkspace};

use constants::TODO_ROW_HEIGHT;

pub use state::TodoTxtState;

fn priority_color(c: char) -> Hsla {
    match c {
        'A' => rgb(0x00EF_4444).into(),
        'B' => rgb(0x00F9_7316).into(),
        'C' => rgb(0x00EA_B308).into(),
        'D' => rgb(0x0022_C55E).into(),
        'E' => rgb(0x003B_82F6).into(),
        _ => rgb(0x009C_A3AF).into(),
    }
}

fn render_pill(label: &str, color: Hsla) -> AnyElement {
    div()
        .h(px(constants::TODO_COL_CHECK))
        .flex()
        .items_center()
        .px(px(constants::TODO_PILL_PADDING_H))
        .rounded(px(constants::TODO_PILL_RADIUS))
        .bg(color.opacity(0.1))
        .text_color(color)
        .text_sm()
        .child(label.to_string())
        .into_any_element()
}

pub fn reload_todo_txt(
    _path: &Path,
    handler: &mut FileHandler,
    _window: &mut Window,
    cx: &mut Context<FileHandler>,
) -> anyhow::Result<ReloadOutcome> {
    let Some(crate::ui::editors::EditorKind::TodoTxt(editor)) = handler.editor.as_ref() else {
        return Ok(ReloadOutcome::Unsupported);
    };
    editor.reload_from_disk(cx)
}

pub struct TodoTxtEditor {
    state: Entity<TodoTxtState>,
}

impl TodoTxtEditor {
    pub const fn new(state: Entity<TodoTxtState>) -> Self {
        Self { state }
    }

    pub fn new_state(path: &Path, window: &mut Window, cx: &mut App) -> Entity<TodoTxtState> {
        let workspace = TodoTxtWorkspace::open(path);
        let total = workspace.task_count();

        cx.new(|cx| {
            let editor_focus = cx.focus_handle();
            let search_input =
                cx.new(|cx| InputState::new(window, cx).placeholder("Search tasks..."));
            let filter_select = cx.new(|cx| {
                SelectState::new(
                    FilterKind::ALL
                        .iter()
                        .map(|f| f.label().to_string())
                        .collect::<Vec<_>>(),
                    Some(IndexPath::new(0)),
                    window,
                    cx,
                )
            });
            let sort_select = cx.new(|cx| {
                SelectState::new(
                    SortKind::ALL
                        .iter()
                        .map(|s| s.label().to_string())
                        .collect::<Vec<_>>(),
                    Some(IndexPath::new(1)),
                    window,
                    cx,
                )
            });
            let new_task_input =
                cx.new(|cx| InputState::new(window, cx).placeholder("Add a task..."));

            let item_sizes = Rc::new(vec![
                Size {
                    width: px(0.0),
                    height: px(TODO_ROW_HEIGHT)
                };
                total
            ]);

            let subscriptions = Self::create_subscriptions(
                &search_input,
                &filter_select,
                &sort_select,
                &new_task_input,
                &editor_focus,
                window,
                cx,
            );

            TodoTxtState {
                workspace,
                priority_picker_open: None,
                pending_focus_desc: None,
                pending_focus_search: false,
                editor_focus,
                search_input,
                filter_select,
                sort_select,
                new_task_input,
                desc_inputs: HashMap::new(),
                date_inputs: HashMap::new(),
                desc_subs: HashMap::new(),
                date_subs: HashMap::new(),
                scroll_handle: VirtualListScrollHandle::new(),
                item_sizes,
                _subscriptions: subscriptions,
            }
        })
    }

    fn create_subscriptions(
        search_input: &Entity<InputState>,
        filter_select: &Entity<SelectState<Vec<String>>>,
        sort_select: &Entity<SelectState<Vec<String>>>,
        new_task_input: &Entity<InputState>,
        editor_focus: &FocusHandle,
        window: &Window,
        cx: &mut Context<TodoTxtState>,
    ) -> Vec<Subscription> {
        let mut subscriptions = Vec::new();

        subscriptions.push(cx.subscribe_in(
            search_input,
            window,
            |this: &mut TodoTxtState, _input, event, _window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.workspace
                        .set_search_query(this.search_input.read(cx).value().to_string());
                    this.refresh_item_sizes();
                    cx.notify();
                }
            },
        ));

        subscriptions.push(cx.subscribe_in(
            filter_select,
            window,
            |this: &mut TodoTxtState, state, _event: &SelectEvent<Vec<String>>, _window, cx| {
                if let Some(index_path) = state.read(cx).selected_index(cx) {
                    this.workspace
                        .set_filter(FilterKind::from_index(index_path.row));
                    this.refresh_item_sizes();
                    cx.notify();
                }
            },
        ));

        subscriptions.push(cx.subscribe_in(
            sort_select,
            window,
            |this: &mut TodoTxtState, state, _event: &SelectEvent<Vec<String>>, _window, cx| {
                if let Some(index_path) = state.read(cx).selected_index(cx) {
                    this.workspace
                        .set_sort(SortKind::from_index(index_path.row));
                    this.refresh_item_sizes();
                    cx.notify();
                }
            },
        ));

        subscriptions.push(cx.subscribe_in(
            new_task_input,
            window,
            |this: &mut TodoTxtState, _input, event, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.add_task(window, cx);
                }
            },
        ));

        let entity = cx.entity();
        let ef = editor_focus.clone();
        subscriptions.push(cx.intercept_keystrokes(move |event, window, cx| {
            if event.keystroke.key.as_str() == "f"
                && event.keystroke.modifiers.secondary()
                && ef.contains_focused(window, cx)
            {
                entity.update(cx, |this, cx| {
                    this.search_input.focus_handle(cx).focus(window, cx);
                });
                cx.stop_propagation();
            }
        }));

        subscriptions
    }

    pub fn render(&self, _cx: &mut App) -> AnyElement {
        div()
            .size_full()
            .child(self.state.clone())
            .into_any_element()
    }

    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.state.read(cx).search_input.focus_handle(cx)
    }

    pub fn reload_from_disk(&self, cx: &mut App) -> anyhow::Result<ReloadOutcome> {
        self.state.update(cx, TodoTxtState::reload_from_disk)
    }
}
