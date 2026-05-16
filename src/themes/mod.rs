use gpui_component::ThemeRegistry;

pub fn load_embedded_themes(cx: &mut gpui::App) {
    let registry = ThemeRegistry::global_mut(cx);

    // From https://github.com/longbridge/gpui-component/tree/main/themes

    let _ = registry.load_themes_from_str(include_str!("../themes/adventure.json"));
    let _ = registry.load_themes_from_str(include_str!("../themes/alduin.json"));
    let _ = registry.load_themes_from_str(include_str!("../themes/asciinema.json"));
    let _ = registry.load_themes_from_str(include_str!("../themes/ayu.json"));
    let _ = registry.load_themes_from_str(include_str!("../themes/catppuccin.json"));
    let _ = registry.load_themes_from_str(include_str!("../themes/everforest.json"));
    let _ = registry.load_themes_from_str(include_str!("../themes/fahrenheit.json"));
    let _ = registry.load_themes_from_str(include_str!("../themes/flexoki.json"));
    let _ = registry.load_themes_from_str(include_str!("../themes/gruvbox.json"));
    let _ = registry.load_themes_from_str(include_str!("../themes/harper.json"));
    let _ = registry.load_themes_from_str(include_str!("../themes/hybrid.json"));
    let _ = registry.load_themes_from_str(include_str!("../themes/jellybeans.json"));
    let _ = registry.load_themes_from_str(include_str!("../themes/kibble.json"));
    let _ = registry.load_themes_from_str(include_str!("../themes/macos-classic.json"));
    let _ = registry.load_themes_from_str(include_str!("../themes/matrix.json"));
    let _ = registry.load_themes_from_str(include_str!("../themes/mellifluous.json"));
    let _ = registry.load_themes_from_str(include_str!("../themes/molokai.json"));
    let _ = registry.load_themes_from_str(include_str!("../themes/solarized.json"));
    let _ = registry.load_themes_from_str(include_str!("../themes/spaceduck.json"));
    let _ = registry.load_themes_from_str(include_str!("../themes/tokyonight.json"));
    let _ = registry.load_themes_from_str(include_str!("../themes/twilight.json"));
}
