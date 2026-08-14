use gpui_component::ThemeRegistry;

pub fn load_embedded_themes(cx: &mut gpui::App) {
    let registry = ThemeRegistry::global_mut(cx);

    // From https://github.com/longbridge/gpui-component/tree/main/themes

    let _ = registry.load_themes_from_str(include_str!("../../../assets/themes/datalith.json"));
    let _ = registry.load_themes_from_str(include_str!("../../../assets/themes/asciinema.json"));
    let _ = registry.load_themes_from_str(include_str!("../../../assets/themes/ayu.json"));
    let _ = registry.load_themes_from_str(include_str!("../../../assets/themes/catppuccin.json"));
    let _ = registry.load_themes_from_str(include_str!("../../../assets/themes/everforest.json"));
    let _ = registry.load_themes_from_str(include_str!("../../../assets/themes/flexoki.json"));
    let _ = registry.load_themes_from_str(include_str!("../../../assets/themes/gruvbox.json"));
    let _ = registry.load_themes_from_str(include_str!("../../../assets/themes/hybrid.json"));
    let _ = registry.load_themes_from_str(include_str!("../../../assets/themes/jellybeans.json"));
    let _ =
        registry.load_themes_from_str(include_str!("../../../assets/themes/macos-classic.json"));
    let _ = registry.load_themes_from_str(include_str!("../../../assets/themes/matrix.json"));
    let _ = registry.load_themes_from_str(include_str!("../../../assets/themes/mellifluous.json"));
    let _ = registry.load_themes_from_str(include_str!("../../../assets/themes/solarized.json"));
    let _ = registry.load_themes_from_str(include_str!("../../../assets/themes/spaceduck.json"));
    let _ = registry.load_themes_from_str(include_str!("../../../assets/themes/tokyonight.json"));
    let _ = registry.load_themes_from_str(include_str!("../../../assets/themes/twilight.json"));
}
