use gpui::{App, KeyBinding};

use super::actions::{
    CloseTab, CopyPath, Delete, Duplicate, FocusSidebar, GoBack, GoForward, NewFile, NewFolder,
    NewTab, OpenInExplorer, OpenLink, Rename, SelectTab1, SelectTab2, SelectTab3, SelectTab4,
    SelectTab5, SelectTab6, SelectTab7, SelectTab8, SelectTab9, ToggleEditorMode,
    ToggleQuickSwitcher, ToggleSearch, ToggleTheme,
};

pub fn register(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-n", NewFile, None),
        KeyBinding::new("cmd-shift-n", NewFolder, None),
        KeyBinding::new("f2", Rename, None),
        KeyBinding::new("cmd-backspace", Delete, None),
        KeyBinding::new("cmd-d", Duplicate, None),
        KeyBinding::new("cmd-shift-e", OpenInExplorer, None),
        KeyBinding::new("cmd-l", CopyPath, None),
        KeyBinding::new("cmd-shift-f", ToggleSearch, None),
        KeyBinding::new("cmd-p", ToggleQuickSwitcher, None),
        KeyBinding::new("cmd-t", NewTab, None),
        KeyBinding::new("cmd-w", CloseTab, None),
        KeyBinding::new("cmd-0", FocusSidebar, None),
        KeyBinding::new("cmd-shift-d", ToggleTheme, None),
        KeyBinding::new("cmd-1", SelectTab1, None),
        KeyBinding::new("cmd-2", SelectTab2, None),
        KeyBinding::new("cmd-3", SelectTab3, None),
        KeyBinding::new("cmd-4", SelectTab4, None),
        KeyBinding::new("cmd-5", SelectTab5, None),
        KeyBinding::new("cmd-6", SelectTab6, None),
        KeyBinding::new("cmd-7", SelectTab7, None),
        KeyBinding::new("cmd-8", SelectTab8, None),
        KeyBinding::new("cmd-9", SelectTab9, None),
        KeyBinding::new("cmd-e", ToggleEditorMode, None),
        KeyBinding::new("cmd-[", GoBack, None),
        KeyBinding::new("cmd-]", GoForward, None),
        KeyBinding::new("cmd-enter", OpenLink, None),
    ]);
}
