#![recursion_limit = "256"]

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

            let docs_vault = match app::docs::ensure_docs_vault() {
                Ok(outcome) => Some(outcome),
                Err(error) => {
                    eprintln!("Failed to seed docs Vault: {error:#}");
                    None
                }
            };
            let (initial_vault, initial_tabs) = match docs_vault {
                Some(outcome) if outcome.first_run => {
                    let tabs = ["Welcome.md", "Tour.todotxt", "Basics.md"]
                        .into_iter()
                        .map(|name| outcome.docs_vault.join(name))
                        .collect();
                    (Some(outcome.docs_vault), tabs)
                }
                _ => (app::settings::snapshot().last_vault, Vec::new()),
            };

            ui::window::open_initial(cx, initial_vault, initial_tabs);
        });
}
