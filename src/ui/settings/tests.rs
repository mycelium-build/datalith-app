use std::path::Path;

use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    App, AppContext as _, ElementId, Entity, InputEvent as _, Pixels, Point, ScrollDelta,
    ScrollWheelEvent, TestAppContext, Window, WindowHandle, point, px, size,
};

use super::SettingsView;
use crate::app::fonts::{self, FontCatalog};
use crate::document::handler::ViewMode;
use crate::ui::DatalithView;

#[test]
fn settings_scroll_does_not_move_the_note_preview() {
    assert_settings_scroll_is_isolated(ViewMode::View);
}

#[test]
fn settings_scroll_does_not_move_the_note_editor() {
    assert_settings_scroll_is_isolated(ViewMode::Edit);
}

fn scroll_at(window: &mut Window, position: Point<Pixels>, delta: f32, cx: &mut App) {
    window.dispatch_event(
        ScrollWheelEvent {
            position,
            delta: ScrollDelta::Pixels(point(px(0.), px(delta))),
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
}

fn open_note(cx: &mut TestAppContext, note: &Path) -> (WindowHandle<Root>, Entity<DatalithView>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        fonts::load_embedded_fonts(cx);
        FontCatalog::init(cx);
        SettingsView::init_theme_options(cx);
        cx.set_global(crate::app::AppState::default());
    });
    let mut app = None;
    let handle = cx.open_window(size(px(1000.), px(800.)), |window, cx| {
        let view = cx.new(|cx| DatalithView::new(false, vec![], window, cx));
        view.update(cx, |view, cx| {
            view.startup_driver = gpui_kit::Task::ready(());
            view.startup = None;
            view.open_file(note.to_path_buf(), true, window, cx);
        });
        app = Some(view.clone());
        Root::new(view, window, cx)
    });
    cx.run_until_parked();
    (handle, app.unwrap())
}

fn offset(origin: Point<Pixels>, x: f32, y: f32) -> Point<Pixels> {
    point(px(f32::from(origin.x) + x), px(f32::from(origin.y) + y))
}

fn assert_settings_scroll_is_isolated(mode: ViewMode) {
    let note = std::env::temp_dir().join(format!(
        "datalith-settings-scroll-{mode:?}-{}.md",
        std::process::id()
    ));
    std::fs::write(&note, "A paragraph in a long note.\n\n".repeat(100)).unwrap();
    let mut cx = TestAppContext::single();
    let (handle, app) = open_note(&mut cx, &note);
    cx.update_window(handle.into(), |_, window, cx| {
        let input = app
            .read(cx)
            .tabs
            .active_handler()
            .unwrap()
            .read(cx)
            .input()
            .unwrap()
            .clone();
        let note_target: ElementId = match mode {
            ViewMode::View => "markdown-title".into(),
            ViewMode::Edit => ("input", input.entity_id()).into(),
        };
        let note_position = |window: &Window, cx: &App| match mode {
            ViewMode::View => window.find("markdown-title").bounds().top(),
            ViewMode::Edit => input.read(cx).scroll_offset().y,
        };
        window.render_frame(cx);
        if mode == ViewMode::View {
            window.click("toggle-mode", cx);
        }
        let initial = note_position(window, cx);
        window.scroll(
            note_target.clone(),
            ScrollDelta::Pixels(point(px(0.), px(-40.))),
            cx,
        );
        let before_settings = note_position(window, cx);
        assert!(
            before_settings < initial,
            "the underlying note must be scrollable"
        );

        app.update(cx, |view, cx| {
            view.settings.open();
            cx.notify();
        });
        window.render_frame(cx);
        let slider_id = (
            "slider",
            app.read(cx).settings.font_size_slider_state.entity_id(),
        );
        let settings_position = window.find(slider_id).bounds().top();
        let close_button = window.find("close-settings").bounds();
        let position = offset(close_button.center(), -120., 250.);
        scroll_at(window, position, -80., cx);
        assert!(
            window.find(slider_id).bounds().top() < settings_position,
            "the settings content must still scroll"
        );
        assert_eq!(
            note_position(window, cx),
            before_settings,
            "scrolling settings must preserve the note's position"
        );

        // Continue scrolling past both ends of the settings content.
        for delta in [-10000., -80., 10000., 80.] {
            scroll_at(window, position, delta, cx);
            assert_eq!(note_position(window, cx), before_settings);
        }
        assert_eq!(window.find(slider_id).bounds().top(), settings_position);

        // The header and the backdrop above the note also isolate wheel input.
        for position in [
            close_button.center(),
            offset(close_button.center(), 60., 250.),
        ] {
            scroll_at(window, position, -80., cx);
            assert_eq!(note_position(window, cx), before_settings);
        }

        window.click("close-settings", cx);
        assert!(!app.read(cx).settings.open);
        assert_eq!(note_position(window, cx), before_settings);
        window.scroll(
            note_target,
            ScrollDelta::Pixels(point(px(0.), px(-40.))),
            cx,
        );
        assert!(
            note_position(window, cx) < before_settings,
            "the note must scroll again after closing settings"
        );
    })
    .unwrap();
    cx.run_until_parked();
    std::fs::remove_file(note).unwrap();
}
