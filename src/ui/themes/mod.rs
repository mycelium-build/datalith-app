use gpui_component::ThemeRegistry;

pub fn load_embedded_themes(cx: &mut gpui::App) {
    let registry = ThemeRegistry::global_mut(cx);

    // From https://github.com/longbridge/gpui-component/tree/main/themes

    let _ = registry.load_themes_from_str(include_str!("datalith.json"));
    let _ = registry.load_themes_from_str(include_str!("asciinema.json"));
    let _ = registry.load_themes_from_str(include_str!("ayu.json"));
    let _ = registry.load_themes_from_str(include_str!("catppuccin.json"));
    let _ = registry.load_themes_from_str(include_str!("everforest.json"));
    let _ = registry.load_themes_from_str(include_str!("flexoki.json"));
    let _ = registry.load_themes_from_str(include_str!("gruvbox.json"));
    let _ = registry.load_themes_from_str(include_str!("hybrid.json"));
    let _ = registry.load_themes_from_str(include_str!("jellybeans.json"));
    let _ = registry.load_themes_from_str(include_str!("macos-classic.json"));
    let _ = registry.load_themes_from_str(include_str!("matrix.json"));
    let _ = registry.load_themes_from_str(include_str!("mellifluous.json"));
    let _ = registry.load_themes_from_str(include_str!("solarized.json"));
    let _ = registry.load_themes_from_str(include_str!("spaceduck.json"));
    let _ = registry.load_themes_from_str(include_str!("tokyonight.json"));
    let _ = registry.load_themes_from_str(include_str!("twilight.json"));
}
