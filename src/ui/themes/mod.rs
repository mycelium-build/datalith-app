use gpui_component::ThemeRegistry;
use gpui_component::notification::Notification;

use crate::ui::notifications;

pub fn load_embedded_themes(cx: &mut gpui::App) -> Vec<Notification> {
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
