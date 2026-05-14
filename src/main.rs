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
        cx.on_action(open_vault);
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
        cx.on_action(handle_close_tab);
        cx.on_action(handle_new_tab);
        cx.on_action(handle_focus_sidebar);
        cx.set_menus([
            Menu::new("File").items([
                MenuItem::action("New File", NewFile),
                MenuItem::action("New Folder", NewFolder),
                MenuItem::separator(),
                MenuItem::action("Rename", Rename),
                MenuItem::action("Delete", Delete),
                MenuItem::action("Duplicate", Duplicate),
                MenuItem::separator(),
                MenuItem::action("Open in Explorer", OpenInExplorer),
                MenuItem::action("Copy Path", CopyPath),
            ]),
            Menu::new("Navigate").items([
                MenuItem::action("Open Vault", OpenVault),
                MenuItem::separator(),
                MenuItem::action("Search Files", ToggleSearch),
                MenuItem::action("Quick Switcher", ToggleQuickSwitcher),
                MenuItem::separator(),
                MenuItem::action("New Tab", NewTab),
                MenuItem::action("Close Tab", CloseTab),
                MenuItem::action("Focus Sidebar", FocusSidebar),
            ]),
        ]);
        cx.bind_keys([
            KeyBinding::new("cmd-shift-f", ToggleSearch, None),
            KeyBinding::new("cmd-p", ToggleQuickSwitcher, None),
            KeyBinding::new("cmd-n", NewFile, None),
            KeyBinding::new("cmd-shift-n", NewFolder, None),
            KeyBinding::new("f2", Rename, None),
            KeyBinding::new("cmd-backspace", Delete, None),
            KeyBinding::new("cmd-d", Duplicate, None),
            KeyBinding::new("cmd-shift-e", OpenInExplorer, None),
            KeyBinding::new("cmd-l", CopyPath, None),
            KeyBinding::new("cmd-w", CloseTab, None),
            KeyBinding::new("cmd-t", NewTab, None),
            KeyBinding::new("cmd-0", FocusSidebar, None),
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
