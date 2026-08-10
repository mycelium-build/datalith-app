use std::ops::Mul;

use conv::ConvAsUtil;
use gpui::{App, px};
use gpui_component::{Theme, ThemeMode, ThemeRegistry};

use crate::app::settings::{self, ColorMode};
use crate::ui::settings::ThemeOptions;

const DEFAULT_LIGHT_THEME: &str = "Datalith Light";
const DEFAULT_DARK_THEME: &str = "Datalith Dark";

fn registered_name<'a>(
    registry: &ThemeRegistry,
    saved: Option<&'a str>,
    default: &'a str,
    mode: ThemeMode,
) -> &'a str {
    let is_valid = |name: &str| {
        registry
            .themes()
            .get(name)
            .is_some_and(|theme| theme.mode == mode)
    };
    saved.filter(|name| is_valid(name)).unwrap_or(default)
}

pub fn apply(cx: &mut App) {
    let settings = settings::snapshot();
    let saved_mode = match settings.color_mode {
        ColorMode::Light => ThemeMode::Light,
        ColorMode::Dark => ThemeMode::Dark,
    };
    let saved_light_name = settings.light_theme_name;
    let saved_dark_name = settings.dark_theme_name;

    let registry = ThemeRegistry::global(cx);
    let light_name = registered_name(
        registry,
        saved_light_name.as_deref(),
        DEFAULT_LIGHT_THEME,
        ThemeMode::Light,
    );
    let dark_name = registered_name(
        registry,
        saved_dark_name.as_deref(),
        DEFAULT_DARK_THEME,
        ThemeMode::Dark,
    );

    let light_theme = registry
        .themes()
        .get(light_name)
        .filter(|theme| theme.mode == ThemeMode::Light)
        .cloned();
    let dark_theme = registry
        .themes()
        .get(dark_name)
        .filter(|theme| theme.mode == ThemeMode::Dark)
        .cloned();

    if let Some(theme) = light_theme {
        Theme::global_mut(cx).light_theme = theme;
        cx.global_mut::<ThemeOptions>().light_theme_name = light_name.into();
    }
    if let Some(theme) = dark_theme {
        Theme::global_mut(cx).dark_theme = theme;
        cx.global_mut::<ThemeOptions>().dark_theme_name = dark_name.into();
    }

    Theme::change(saved_mode, None, cx);
    Theme::global_mut(cx).mode = saved_mode;
    Theme::global_mut(cx).font_size =
        px(crate::ui::BASE_FONT_SIZE.mul(settings.font_scale.approx().unwrap_or(1.0)));

    cx.refresh_windows();
}
