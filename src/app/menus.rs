use gpui_kit::component::GlobalState;
use gpui_kit::{App, Menu, MenuItem};

use crate::app::actions::{OpenAbout, OpenDocumentation, OpenSettings, OpenShortcuts};

use super::actions::{
    CloseTab, CopyPath, Delete, Duplicate, FocusSidebar, GoBack, GoForward, NewFile, NewFolder,
    NewTab, OpenInExplorer, OpenVault, Quit, Rename, ToggleQuickSwitcher, ToggleSearch,
    ToggleTheme,
};

pub fn install(cx: &mut App) {
    cx.set_menus([
        application_menu(cx),
        file_menu(),
        navigate_menu(),
        help_menu(),
    ]);

    if let Some(menus) = cx.get_menus() {
        GlobalState::global_mut(cx).set_app_menus(menus);
    }
}

fn application_menu(cx: &App) -> Menu {
    let mut items = vec![MenuItem::action(
        format!(
            "About {}",
            crate::channel::Channel::current().product_name()
        ),
        OpenAbout,
    )];
    if super::update::Updater::get(cx).is_some() {
        items.push(MenuItem::action(
            "Check for updates",
            super::actions::CheckForUpdates,
        ));
    }
    items.extend([
        MenuItem::separator(),
        MenuItem::action("Settings", OpenSettings),
        MenuItem::action("Shortcuts list", OpenShortcuts),
        MenuItem::separator(),
        MenuItem::action(
            format!("Quit {}", crate::channel::Channel::current().product_name()),
            Quit,
        ),
    ]);
    Menu::new(crate::channel::Channel::current().product_name()).items(items)
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
    Menu::new("Help").items([MenuItem::action(
        "Datalith Documentation",
        OpenDocumentation,
    )])
}
