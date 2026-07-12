use gpui::*;

use super::actions::*;

pub(crate) fn install(cx: &mut App) {
    cx.set_menus([file_menu(), navigate_menu()]);
}

fn file_menu() -> Menu {
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
    ])
}

fn navigate_menu() -> Menu {
    Menu::new("Navigate").items([
        MenuItem::action("Open Vault", OpenVault),
        MenuItem::separator(),
        MenuItem::action("Search Files", ToggleSearch),
        MenuItem::action("Quick Switcher", ToggleQuickSwitcher),
        MenuItem::action("Focus Sidebar", FocusSidebar),
        MenuItem::separator(),
        MenuItem::action("New Tab", NewTab),
        MenuItem::action("Close Tab", CloseTab),
        MenuItem::action("Go Back", GoBack),
        MenuItem::action("Go Forward", GoForward),
        MenuItem::separator(),
        MenuItem::action("Toggle Dark Mode", ToggleTheme),
        MenuItem::separator(),
        MenuItem::action("Settings", OpenSettings),
    ])
}
