use gpui::{App, Menu, MenuItem};
use gpui_component::GlobalState;

use crate::app::actions::{OpenAbout, OpenLicenses, OpenSettings, OpenShortcuts, OpenSource};

use super::actions::{
    CloseTab, CopyPath, Delete, Duplicate, FocusSidebar, GoBack, GoForward, NewFile, NewFolder,
    NewTab, OpenInExplorer, OpenVault, Quit, Rename, ToggleQuickSwitcher, ToggleSearch,
    ToggleTheme,
};

pub fn install(cx: &mut App) {
    cx.set_menus([
        application_menu(),
        file_menu(),
        navigate_menu(),
        help_menu(),
    ]);

    if let Some(menus) = cx.get_menus() {
        GlobalState::global_mut(cx).set_app_menus(menus);
    }
}

fn application_menu() -> Menu {
    Menu::new("Datalith").items([
        MenuItem::action("About Datalith", OpenAbout),
        MenuItem::separator(),
        MenuItem::action("Settings", OpenSettings),
        MenuItem::action("Shortcuts list", OpenShortcuts),
        MenuItem::separator(),
        MenuItem::action("Quit Datalith", Quit),
    ])
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
    ])
}

fn help_menu() -> Menu {
    Menu::new("Help").items([
        MenuItem::action("View Dependency Licenses", OpenLicenses),
        MenuItem::action("View Corresponding Source", OpenSource),
    ])
}
