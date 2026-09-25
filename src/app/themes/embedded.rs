use super::{ThemeDocument, storage::StoredThemeSet};
use crate::ui::notifications;
use gpui_kit::component::{ThemeRegistry, notification::Notification};

// From https://github.com/longbridge/gpui-component/tree/main/themes
const THEME_SETS: &[(&str, &str)] = &[
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
];

pub(super) fn load(cx: &mut gpui_kit::App) -> Vec<Notification> {
    let registry = ThemeRegistry::global_mut(cx);
    THEME_SETS
        .iter()
        .filter_map(|(name, content)| {
            registry
                .load_themes_from_str(content)
                .err()
                .map(|error| notifications::theme_load_failed(name, &error))
        })
        .collect()
}

pub(super) fn sets() -> impl Iterator<Item = StoredThemeSet> {
    THEME_SETS
        .iter()
        .filter_map(|(_, content)| serde_json::from_str(content).ok())
}

#[allow(
    clippy::expect_used,
    reason = "the bundled Datalith asset and its two variants are release-time invariants, verified by domain tests"
)]
pub(super) fn defaults() -> [ThemeDocument; 2] {
    let set: StoredThemeSet =
        serde_json::from_str(include_str!("../../../assets/themes/datalith.json"))
            .expect("Bundled Datalith theme must parse");
    let light = set
        .themes
        .iter()
        .find(|v| v.name() == "Datalith Light")
        .expect("Datalith Light must exist")
        .clone();
    let dark = set
        .themes
        .iter()
        .find(|v| v.name() == "Datalith Dark")
        .expect("Datalith Dark must exist")
        .clone();
    [light, dark]
}
