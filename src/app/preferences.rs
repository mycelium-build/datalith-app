use gpui::*;
use gpui_component::{Theme, ThemeMode, ThemeRegistry};

use crate::app::settings::{self, ColorMode};
use crate::ui::settings::ThemeOptions;

pub(crate) fn apply(cx: &mut App) {
    let settings = settings::snapshot();
    let saved_mode = match settings.color_mode {
        ColorMode::Light => ThemeMode::Light,
        ColorMode::Dark => ThemeMode::Dark,
    };
    let saved_light_name = settings.light_theme_name;
    let saved_dark_name = settings.dark_theme_name;

    let registry = ThemeRegistry::global(cx);
    let light_theme = saved_light_name.as_deref().and_then(|name| {
        registry
            .themes()
            .get(name)
            .filter(|theme| theme.mode == ThemeMode::Light)
            .cloned()
    });
    let dark_theme = saved_dark_name.as_deref().and_then(|name| {
        registry
            .themes()
            .get(name)
            .filter(|theme| theme.mode == ThemeMode::Dark)
            .cloned()
    });
    let _ = registry;

    if let Some(theme) = light_theme {
        Theme::global_mut(cx).light_theme = theme;
        cx.global_mut::<ThemeOptions>().light_theme_name = saved_light_name.unwrap().into();
    }
    if let Some(theme) = dark_theme {
        Theme::global_mut(cx).dark_theme = theme;
        cx.global_mut::<ThemeOptions>().dark_theme_name = saved_dark_name.unwrap().into();
    }

    Theme::change(saved_mode, None, cx);
    Theme::global_mut(cx).mode = saved_mode;

    Theme::global_mut(cx).font_size =
        px(crate::ui::BASE_FONT_SIZE as f32 * settings.font_scale as f32);
    cx.refresh_windows();
}
