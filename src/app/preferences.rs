use gpui_kit::App;
use gpui_kit::component::Theme;

use crate::app::settings::ThemePreference;

/// Applies a [`ThemePreference`] to the current session:
/// pins or clears the native window appearance,
/// resolves the effective mode against the resulting system appearance,
/// and repaints all windows.
pub fn apply_theme_preference(preference: ThemePreference, cx: &mut App) {
    cx.set_window_appearance(preference.to_window_appearance());
    let effective = preference.resolve(cx.window_appearance()).into();
    Theme::change(effective, None, cx);
    Theme::global_mut(cx).mode = effective;
    super::fonts::apply(cx);
    cx.refresh_windows();
}
