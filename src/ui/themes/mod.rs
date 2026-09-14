use gpui_kit::component::ThemeRegistry;
use gpui_kit::component::notification::Notification;

use crate::ui::notifications;

pub fn load_embedded_themes(cx: &mut gpui_kit::App) -> Vec<Notification> {
    let registry = ThemeRegistry::global_mut(cx);

    // From https://github.com/longbridge/gpui-component/tree/main/themes
    [
        (
            "Datalith",
            include_str!("../../../assets/themes/datalith.json"),
        ),
        (
            "Asciinema",
            include_str!("../../../assets/themes/asciinema.json"),
        ),
        ("Ayu", include_str!("../../../assets/themes/ayu.json")),
        (
            "Catppuccin",
            include_str!("../../../assets/themes/catppuccin.json"),
        ),
        (
            "Everforest",
            include_str!("../../../assets/themes/everforest.json"),
        ),
        (
            "Flexoki",
            include_str!("../../../assets/themes/flexoki.json"),
        ),
        (
            "Gruvbox",
            include_str!("../../../assets/themes/gruvbox.json"),
        ),
        ("Hybrid", include_str!("../../../assets/themes/hybrid.json")),
        (
            "Jellybeans",
            include_str!("../../../assets/themes/jellybeans.json"),
        ),
        (
            "macOS Classic",
            include_str!("../../../assets/themes/macos-classic.json"),
        ),
        ("Matrix", include_str!("../../../assets/themes/matrix.json")),
        (
            "Mellifluous",
            include_str!("../../../assets/themes/mellifluous.json"),
        ),
        (
            "Solarized",
            include_str!("../../../assets/themes/solarized.json"),
        ),
        (
            "Spaceduck",
            include_str!("../../../assets/themes/spaceduck.json"),
        ),
        (
            "Tokyo Night",
            include_str!("../../../assets/themes/tokyonight.json"),
        ),
        (
            "Twilight",
            include_str!("../../../assets/themes/twilight.json"),
        ),
    ]
    .into_iter()
    .filter_map(|(name, content)| {
        registry
            .load_themes_from_str(content)
            .err()
            .map(|error| notifications::theme_load_failed(name, &error))
    })
    .collect()
}

pub fn logo_color(theme: &gpui_kit::component::Theme) -> gpui_kit::Hsla {
    match crate::channel::Channel::current() {
        crate::channel::Channel::Stable => theme.primary,
        crate::channel::Channel::Preview => gpui_kit::rgb(0xe8_b9_20).into(),
        crate::channel::Channel::Dev => gpui_kit::rgb(0x30_ba_78).into(),
    }
}
