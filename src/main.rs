mod app;
mod document;
mod ui;
mod vault;

fn main() {
    gpui_platform::application()
        .with_assets(app::assets::DatalithAssets)
        .run(|cx| {
            gpui_component::init(cx);
            ui::themes::load_embedded_themes(cx);
            ui::settings::SettingsView::init_theme_options(cx);

            app::preferences::apply(cx);
            cx.set_global(app::AppState::default());
            app::actions::register(cx);
            app::keymap::register(cx);
            app::menus::install(cx);

            ui::window::open_initial(cx, app::config::load_last_folder());
        });
}
