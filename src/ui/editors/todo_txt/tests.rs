use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext as _, Focusable as _, TestAppContext, px, size};

use super::TodoTxtEditor;

const PREVIEW_TASK: &str = "(A) 2026-09-25 Keep preview task unchanged @work\n";

#[gpui_kit::test]
fn preview_task_description_date_and_completion_are_read_only(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);

    let mut editor_state = None;
    let window = cx.open_window(size(px(720.), px(360.)), |window, cx| {
        let state = TodoTxtEditor::preview_state(PREVIEW_TASK, window, cx)
            .expect("preview task should parse");
        editor_state = Some(state.clone());
        Root::new(state, window, cx)
    });
    let editor_state = editor_state.expect("preview state");

    cx.update_window(window.into(), |_, window, cx| {
        window.render_frame(cx);

        let (description_before, date_before) = {
            let editor = editor_state.read(cx);
            (
                editor.desc_inputs[&0].read(cx).value().to_string(),
                editor.date_inputs[&0].read(cx).value().to_string(),
            )
        };
        let model_before = {
            let editor = editor_state.read(cx);
            let task = editor.workspace.task(0).expect("preview task row");
            (
                task.description.clone(),
                task.creation_date,
                task.completed,
                editor.workspace.completed_count(),
            )
        };

        let description = editor_state.read(cx).desc_inputs[&0].clone();
        let date = editor_state.read(cx).date_inputs[&0].clone();
        description.focus_handle(cx).focus(window, cx);
        window.render_frame(cx);
        window.input("Unauthorized edit", cx);
        date.focus_handle(cx).focus(window, cx);
        window.render_frame(cx);
        window.input("2030-01-01", cx);
        window.click(("todo-check", 0_usize), cx);
        window.render_frame(cx);

        let editor = editor_state.read(cx);
        let task = editor.workspace.task(0).expect("preview task row");
        assert_eq!(task.description, model_before.0);
        assert_eq!(task.creation_date, model_before.1);
        assert_eq!(task.completed, model_before.2);
        assert_eq!(editor.workspace.completed_count(), model_before.3);
        assert_eq!(editor.desc_inputs[&0].read(cx).value(), description_before);
        assert_eq!(editor.date_inputs[&0].read(cx).value(), date_before);
        assert_eq!(window.find(("todo-check", 0_usize)).checked(), Some(false));
    })
    .expect("preview window should remain open");
}
