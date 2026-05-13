mod actions;
mod app;
mod config;
mod filetree;
mod search;
mod view;

use gpui::*;
use gpui_component::Root;

use crate::actions::*;
use crate::app::AppState;
use crate::config::load_last_folder;
use crate::view::DatalithView;

fn main() {
    let app = gpui_platform::application().with_assets(gpui_component_assets::Assets);

    app.run(move |cx| {
        gpui_component::init(cx);

        cx.set_global(AppState { view: None });
        cx.on_action(open_codex);
        cx.on_action(toggle_search);
        cx.on_action(toggle_quick_switcher);
        cx.on_action(close_palette);
        cx.on_action(handle_new_file);
        cx.on_action(handle_new_folder);
        cx.on_action(handle_rename);
        cx.on_action(handle_delete);
        cx.on_action(handle_duplicate);
        cx.on_action(handle_open_in_explorer);
        cx.on_action(handle_copy_path);
        cx.set_menus([Menu::new("File").items([
            MenuItem::action("Open codex", OpenCodex),
            MenuItem::action("Search files...", ToggleSearch),
            MenuItem::action("Quick Switcher...", ToggleQuickSwitcher),
        ])]);
        cx.bind_keys([
            KeyBinding::new("cmd-shift-f", ToggleSearch, None),
            KeyBinding::new("cmd-p", ToggleQuickSwitcher, None),
        ]);

        let last_folder = load_last_folder();

        cx.spawn(async move |cx| {
            cx.open_window(WindowOptions::default(), |window, cx| {
                let view = cx.new(|cx| DatalithView::new(window, cx));
                cx.update_global(|state: &mut AppState, _| {
                    state.view = Some(view.clone());
                });
                if let Some(ref path) = last_folder {
                    view.update(cx, |view, cx| {
                        view.set_root_path(path.clone(), cx);
                    });
                }
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("Failed to open window");
        })
        .detach();
    });
}
