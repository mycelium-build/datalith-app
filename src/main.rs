#![recursion_limit = "256"]
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::indexing_slicing,
        clippy::string_slice
    )
)]

mod app;
mod document;
mod ui;
mod vault;

fn main() {
    gpui_platform::application()
        .with_assets(app::assets::DatalithAssets)
        .run(|cx| {
            gpui_component::init(cx);
            app::fonts::load_embedded_fonts(cx);
            ui::themes::load_embedded_themes(cx);
            ui::settings::SettingsView::init_theme_options(cx);

            let pending_notifications = app::preferences::apply(cx);
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
            let first_startup = docs_vault
                .as_ref()
                .is_some_and(|outcome| outcome.first_run);
            let (initial_vault, initial_tabs) = match docs_vault {
                Some(outcome) if outcome.first_run => {
                    let tabs = app::docs::INITIAL_TABS
                        .iter()
                        .map(|name| outcome.docs_vault.join(name))
                        .collect();
                    (Some(outcome.docs_vault), tabs)
                }
                _ => (app::settings::snapshot().last_vault, Vec::new()),
            };

            ui::window::open_initial(
                cx,
                first_startup,
                initial_vault,
                initial_tabs,
                pending_notifications,
            );
        });
}
