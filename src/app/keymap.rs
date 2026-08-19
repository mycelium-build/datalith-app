use gpui::{App, KeyBinding, Keystroke};

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

pub fn display_binding(binding: &str) -> String {
    Keystroke::parse(binding).map_or_else(|_| binding.to_owned(), |keystroke| keystroke.to_string())
}

shortcuts!(
    // `secondary` resolves to Command on macOS and Ctrl on Linux/Windows.
    // File
    ("File", "secondary-q", "Quit Datalith", Quit),
    ("File", "secondary-n", "New note", NewFile),
    ("File", "secondary-shift-n", "New folder", NewFolder),
    ("File", "f2", "Rename", Rename),
    ("File", "secondary-backspace", "Delete", Delete),
    ("File", "secondary-d", "Duplicate", Duplicate),
    (
        "File",
        "secondary-shift-e",
        "Open in Explorer",
        OpenInExplorer
    ),
    ("File", "secondary-l", "Copy path", CopyPath),
    // Navigation
    (
        "Navigation",
        "secondary-p",
        "Quick switcher",
        ToggleQuickSwitcher
    ),
    ("Navigation", "secondary-0", "Focus sidebar", FocusSidebar),
    ("Navigation", "secondary-[", "Navigate back", GoBack),
    ("Navigation", "secondary-]", "Navigate forward", GoForward),
    ("Navigation", "secondary-enter", "Open link", OpenLink),
    // Tabs
    ("Tabs", "secondary-t", "New tab", NewTab),
    ("Tabs", "secondary-w", "Close tab", CloseTab),
    ("Tabs", "secondary-1", "Select tab", SelectTab1),
    ("Tabs", "secondary-2", "Select tab", SelectTab2),
    ("Tabs", "secondary-3", "Select tab", SelectTab3),
    ("Tabs", "secondary-4", "Select tab", SelectTab4),
    ("Tabs", "secondary-5", "Select tab", SelectTab5),
    ("Tabs", "secondary-6", "Select tab", SelectTab6),
    ("Tabs", "secondary-7", "Select tab", SelectTab7),
    ("Tabs", "secondary-8", "Select tab", SelectTab8),
    ("Tabs", "secondary-9", "Select last tab", SelectLastTab),
    // View
    ("View", "secondary-shift-f", "Search files", ToggleSearch),
    (
        "View",
        "secondary-e",
        "Toggle edit / view",
        ToggleEditorMode
    ),
    ("View", "secondary-shift-d", "Toggle theme", ToggleTheme),
    ("View", "secondary-,", "Open settings", OpenSettings),
    // Help
    ("Help", "secondary-/", "Show shortcuts", OpenShortcuts),
);

#[cfg(test)]
mod tests {
    use super::display_binding;

    #[test]
    fn displays_secondary_with_platform_conventions() {
        let expected = if cfg!(target_os = "macos") {
            "⌘⇧F"
        } else {
            "ctrl-shift-f"
        };

        assert_eq!(display_binding("secondary-shift-f"), expected);
    }
}
