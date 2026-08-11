use gpui::{App, Menu, MenuItem};

use crate::app::actions::OpenShortcuts;

use super::actions::{
    CloseTab, CopyPath, Delete, Duplicate, FocusSidebar, GoBack, GoForward, NewFile, NewFolder,
    NewTab, OpenInExplorer, OpenSettings, OpenVault, Quit, Rename, ToggleQuickSwitcher,
    ToggleSearch, ToggleTheme,
};

pub fn install(cx: &App) {
    cx.set_menus([application_menu(), file_menu(), navigate_menu()]);
}

fn application_menu() -> Menu {
    Menu::new("Datalith").items([MenuItem::action("Quit Datalith", Quit)])
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
        MenuItem::action("Shortcuts list", OpenShortcuts),
    ])
}

#[cfg(test)]
mod tests {
    use gpui::Action;

    use super::*;
    use crate::app::actions::Quit;

    #[test]
    fn application_menu_contains_quit_action() {
        let contains_quit = application_menu().items.into_iter().any(|item| {
            matches!(
                item,
                MenuItem::Action { action, .. } if action.name() == Quit.name()
            )
        });

        assert!(contains_quit, "application menu should contain Quit");
    }
}
