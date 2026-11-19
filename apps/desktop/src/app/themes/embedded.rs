use super::{ThemeDocument, model::ThemeModes, storage::StoredThemeSet};

use datalith_ui::themes::THEME_SETS;

pub(super) fn sets() -> impl Iterator<Item = StoredThemeSet> {
    THEME_SETS
        .iter()
        .filter_map(|content| serde_json::from_str(content).ok())
}

#[allow(
    clippy::expect_used,
    reason = "bundled JSON and Light/Dark theme variants are invariants covered by domain tests"
)]
pub(super) fn defaults() -> ThemeModes<ThemeDocument> {
    let set: StoredThemeSet =
        serde_json::from_str(include_str!("../../../../../assets/themes/datalith.json"))
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
    ThemeModes::new(light, dark)
}
