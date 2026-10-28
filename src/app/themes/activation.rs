use std::rc::Rc;

use gpui_kit::App;
use gpui_kit::component::Theme;

use super::ThemeLibrary;
use crate::app::{
    preferences,
    settings::{self, ThemeKind},
};

/// Applies the library's validated slots and the session's appearance preference.
pub fn refresh_current(cx: &mut App) {
    let library = cx.global::<ThemeLibrary>();
    let light = library
        .resolved_config(library.current(ThemeKind::Light))
        .map(Rc::new);
    let dark = library
        .resolved_config(library.current(ThemeKind::Dark))
        .map(Rc::new);
    if let Some(light) = light {
        Theme::global_mut(cx).light_theme = light;
    }
    if let Some(dark) = dark {
        Theme::global_mut(cx).dark_theme = dark;
    }
    preferences::apply_theme_preference(settings::snapshot().theme_preference, cx);
}
