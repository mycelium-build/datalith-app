mod actions;
mod app;
mod config;
mod consts;
mod fs_ops;
mod markdown;
mod search;
mod themes;
mod utils;
mod view;

use gpui::*;
use gpui_component::{Root, Theme, ThemeMode, ThemeRegistry};

use crate::actions::*;
use crate::app::AppState;
use crate::config::{
    load_dark_theme_name, load_font_size_multiplier, load_last_folder, load_light_theme_name,
    load_theme_mode,
};
use crate::view::DatalithView;
use crate::view::settings::{SettingsView, ThemeOptions};

fn main() {
    let app = gpui_platform::application().with_assets(gpui_component_assets::Assets);

    app.run(move |cx| {
        gpui_component::init(cx);
        themes::load_embedded_themes(cx);
        SettingsView::init_theme_options(cx);

        let saved_mode = load_theme_mode().unwrap_or(ThemeMode::Light);
        let saved_light_name = load_light_theme_name();
        let saved_dark_name = load_dark_theme_name();
        let light_theme_name = saved_light_name.clone().unwrap_or_default();
        let dark_theme_name = saved_dark_name.clone().unwrap_or_default();
        let light_valid = saved_light_name
            .as_ref()
            .map(|n| ThemeRegistry::global(cx).themes().get(n.as_str()).is_some())
            .unwrap_or(false);
        let dark_valid = saved_dark_name
            .as_ref()
            .map(|n| ThemeRegistry::global(cx).themes().get(n.as_str()).is_some())
            .unwrap_or(false);
        let registry = ThemeRegistry::global(cx);
        let light_theme_opt = registry
            .themes()
            .get(light_theme_name.as_str())
            .filter(|t| t.mode == ThemeMode::Light)
            .cloned();
        let dark_theme_opt = registry
            .themes()
            .get(dark_theme_name.as_str())
            .filter(|t| t.mode == ThemeMode::Dark)
            .cloned();
        let _ = registry;
        if let Some(theme_config) = light_theme_opt {
            Theme::global_mut(cx).light_theme = theme_config;
        }
        if let Some(theme_config) = dark_theme_opt {
            Theme::global_mut(cx).dark_theme = theme_config;
        }
        Theme::change(saved_mode, None, cx);
        Theme::global_mut(cx).mode = saved_mode;
        cx.refresh_windows();

        if light_valid {
            cx.global_mut::<ThemeOptions>().light_theme_name = saved_light_name.unwrap().into();
        }
        if dark_valid {
            cx.global_mut::<ThemeOptions>().dark_theme_name = saved_dark_name.unwrap().into();
        }

        if let Some(multiplier) = load_font_size_multiplier() {
            gpui_component::Theme::global_mut(cx).font_size =
                gpui::px(crate::consts::BASE_FONT_SIZE as f32 * multiplier as f32);
        }

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
        cx.on_action(toggle_theme);
        cx.on_action(handle_select_tab_1);
        cx.on_action(handle_select_tab_2);
        cx.on_action(handle_select_tab_3);
        cx.on_action(handle_select_tab_4);
        cx.on_action(handle_select_tab_5);
        cx.on_action(handle_select_tab_6);
        cx.on_action(handle_select_tab_7);
        cx.on_action(handle_select_tab_8);
        cx.on_action(handle_select_tab_9);
        cx.on_action(open_settings);
        cx.on_action(toggle_editor_mode);
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
                MenuItem::separator(),
                MenuItem::action("Toggle Dark Mode", ToggleTheme),
                MenuItem::separator(),
                MenuItem::action("Settings", OpenSettings),
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
