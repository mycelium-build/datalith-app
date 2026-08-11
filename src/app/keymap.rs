use gpui::{App, KeyBinding};

use super::actions::{
    CloseTab, CopyPath, Delete, Duplicate, FocusSidebar, GoBack, GoForward, NewFile, NewFolder,
    NewTab, OpenInExplorer, OpenLink, OpenSettings, OpenShortcuts, Quit, Rename, SelectLastTab,
    SelectTab1, SelectTab2, SelectTab3, SelectTab4, SelectTab5, SelectTab6, SelectTab7, SelectTab8,
    ToggleEditorMode, ToggleQuickSwitcher, ToggleSearch, ToggleTheme,
};

macro_rules! shortcuts {
    ($(($category:literal, $keys:literal, $description:literal, $action:ident)),* $(,)?) => {
        pub fn register(cx: &mut App) {
            cx.bind_keys([
                $( KeyBinding::new($keys, $action, None), )*
            ]);
        }

        /// `(category, keys, description)` for every registered shortcut.
        #[must_use]
        pub fn shortcut_descriptions() -> Vec<(&'static str, &'static str, &'static str)> {
            vec![
                $( ($category, $keys, $description), )*
            ]
        }
    };
}

shortcuts!(
    // File
    ("File", "cmd-q", "Quit Datalith", Quit),
    ("File", "cmd-n", "New note", NewFile),
    ("File", "cmd-shift-n", "New folder", NewFolder),
    ("File", "f2", "Rename", Rename),
    ("File", "cmd-backspace", "Delete", Delete),
    ("File", "cmd-d", "Duplicate", Duplicate),
    ("File", "cmd-shift-e", "Open in Explorer", OpenInExplorer),
    ("File", "cmd-l", "Copy path", CopyPath),
    // Navigation
    ("Navigation", "cmd-p", "Quick switcher", ToggleQuickSwitcher),
    ("Navigation", "cmd-0", "Focus sidebar", FocusSidebar),
    ("Navigation", "cmd-[", "Navigate back", GoBack),
    ("Navigation", "cmd-]", "Navigate forward", GoForward),
    ("Navigation", "cmd-enter", "Open link", OpenLink),
    // Tabs
    ("Tabs", "cmd-t", "New tab", NewTab),
    ("Tabs", "cmd-w", "Close tab", CloseTab),
    ("Tabs", "cmd-1", "Select tab", SelectTab1),
    ("Tabs", "cmd-2", "Select tab", SelectTab2),
    ("Tabs", "cmd-3", "Select tab", SelectTab3),
    ("Tabs", "cmd-4", "Select tab", SelectTab4),
    ("Tabs", "cmd-5", "Select tab", SelectTab5),
    ("Tabs", "cmd-6", "Select tab", SelectTab6),
    ("Tabs", "cmd-7", "Select tab", SelectTab7),
    ("Tabs", "cmd-8", "Select tab", SelectTab8),
    ("Tabs", "cmd-9", "Select last tab", SelectLastTab),
    // View
    ("View", "cmd-shift-f", "Search files", ToggleSearch),
    ("View", "cmd-e", "Toggle edit / view", ToggleEditorMode),
    ("View", "cmd-shift-d", "Toggle theme", ToggleTheme),
    ("View", "cmd-,", "Open settings", OpenSettings),
    // Help
    ("Help", "cmd-/", "Show shortcuts", OpenShortcuts),
);
