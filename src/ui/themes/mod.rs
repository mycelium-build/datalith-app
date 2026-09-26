mod editor;
pub mod preview;
pub use editor::ThemeEditor;

use gpui_kit::App;

use crate::app::{
    settings::{ThemeKind, ThemePreference},
    themes::{self, ThemeLibrary},
};
use crate::ui::notifications;

/// Only the About mark identifies the release channel by color.
pub fn about_logo_color(theme: &gpui_kit::component::Theme) -> gpui_kit::Hsla {
    match crate::channel::Channel::current() {
        crate::channel::Channel::Stable => theme.primary,
        crate::channel::Channel::Preview => gpui_kit::rgb(0xe8_b9_20).into(),
        crate::channel::Channel::Dev => gpui_kit::rgb(0x30_ba_78).into(),
    }
}

pub fn set_current(id: u64, kind: ThemeKind, cx: &mut App) {
    if let Err(error) = cx.global_mut::<ThemeLibrary>().set_current(id, kind) {
        notifications::push_window_notification(
            cx,
            notifications::settings_save_failed("theme", &error),
        );
        return;
    }
    themes::refresh_current(cx);
    crate::ui::settings::SettingsView::init_theme_options(cx);
}

pub fn change_mode(preference: ThemePreference, cx: &mut App) {
    if let Err(error) = crate::app::settings::set_theme_preference(preference) {
        notifications::push_window_notification(
            cx,
            notifications::settings_save_failed("theme mode", &error),
        );
        return;
    }
    crate::app::preferences::apply_theme_preference(preference, cx);
    crate::ui::settings::SettingsView::init_theme_options(cx);
}
