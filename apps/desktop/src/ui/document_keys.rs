use datalith_ui::contexts::{CELL_EDITOR_CONTEXT, DATE_CONTEXT, PRIORITY_CONTEXT, SCROLL_CONTEXT};
use gpui_kit::base::actions::{
    Cancel, SelectDown, SelectFirst, SelectLast, SelectLeft, SelectPageDown, SelectPageUp,
    SelectRight, SelectUp,
};
use gpui_kit::{App, KeyBinding};

/// Fixed navigation and editing keys for the todo list.
pub(super) fn bind_todo_keys(cx: &mut App) {
    use datalith_ui::actions::{NextTask, PreviousTask};
    use datalith_ui::contexts::TODO_LIST_CONTEXT;

    let task_fields = format!("({TODO_LIST_CONTEXT} > Input) || {TODO_LIST_CONTEXT}");
    cx.bind_keys([
        KeyBinding::new("up", PreviousTask, Some(&task_fields)),
        KeyBinding::new("down", NextTask, Some(&task_fields)),
        KeyBinding::new("up", SelectUp, Some(PRIORITY_CONTEXT)),
        KeyBinding::new("down", SelectDown, Some(PRIORITY_CONTEXT)),
        KeyBinding::new("escape", Cancel, Some(DATE_CONTEXT)),
    ]);
}

/// Binds Escape to `Cancel` in a cell editor, after the field's own Escape,
/// and in the viewer; binds the arrows around a graph.
pub(super) fn bind_base_keys(cx: &mut App) {
    let graph = Some(datalith_ui::contexts::BASE_GRAPH_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("escape", Cancel, Some(CELL_EDITOR_CONTEXT)),
        KeyBinding::new(
            "escape",
            Cancel,
            Some(datalith_ui::contexts::BASE_VIEWER_CONTEXT),
        ),
        KeyBinding::new("up", SelectUp, graph),
        KeyBinding::new("down", SelectDown, graph),
        KeyBinding::new("left", SelectLeft, graph),
        KeyBinding::new("right", SelectRight, graph),
    ]);
}

pub(super) fn bind_scroll_keys(cx: &mut App) {
    let context = Some(SCROLL_CONTEXT);
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
