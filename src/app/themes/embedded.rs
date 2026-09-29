use super::{ThemeDocument, storage::StoredThemeSet};

// From https://github.com/longbridge/gpui-component/tree/main/themes
const THEME_SETS: &[&str] = &[
    include_str!("../../../assets/themes/datalith.json"),
    include_str!("../../../assets/themes/asciinema.json"),
    include_str!("../../../assets/themes/ayu.json"),
    include_str!("../../../assets/themes/catppuccin.json"),
    include_str!("../../../assets/themes/everforest.json"),
    include_str!("../../../assets/themes/flexoki.json"),
    include_str!("../../../assets/themes/gruvbox.json"),
    include_str!("../../../assets/themes/hybrid.json"),
    include_str!("../../../assets/themes/jellybeans.json"),
    include_str!("../../../assets/themes/macos-classic.json"),
    include_str!("../../../assets/themes/matrix.json"),
    include_str!("../../../assets/themes/mellifluous.json"),
    include_str!("../../../assets/themes/solarized.json"),
    include_str!("../../../assets/themes/spaceduck.json"),
    include_str!("../../../assets/themes/tokyonight.json"),
    include_str!("../../../assets/themes/twilight.json"),
];

pub(super) fn sets() -> impl Iterator<Item = StoredThemeSet> {
    THEME_SETS
        .iter()
        .filter_map(|content| serde_json::from_str(content).ok())
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
