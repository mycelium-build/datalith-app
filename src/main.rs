#![deny(warnings)]
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
mod channel;
mod document;
mod server;
mod ui;
mod vault;

fn main() {
    let channel = channel::Channel::current();
    if std::env::args().any(|arg| arg == "--print-build-identity") {
        println!("{}", app::version::build_identity());
        return;
    }
    let application = gpui_kit::application().with_assets(app::assets::DatalithAssets);
    application.on_open_urls(app::deeplink::capture);
    application.run(move |cx| {
        cx.set_app_identity(channel.identifier(), channel.product_name());
        app::init(cx);
        let mut pending_notifications = app::fonts::load_embedded_fonts(cx);
        pending_notifications.extend(ui::themes::load_embedded_themes(cx));
        ui::settings::SettingsView::init_theme_options(cx);

        pending_notifications.extend(app::preferences::apply(cx));
        cx.set_global(app::AppState::default());
        app::actions::register(cx);
        app::keymap::register(cx);
        app::menus::install(cx);

        let settings = app::settings::snapshot();
        let first_startup = !settings.onboarding_complete;
        let root = settings
            .last_vault
            .or_else(|| first_startup.then(app::docs::docs_vault_path));

        app::deeplink::start(cx);
        if let Err(error) = server::sync() {
            eprintln!("Failed to start Local Server: {error}");
            let server_settings = app::settings::snapshot().server;
            pending_notifications.push(ui::notifications::server_start_failed(
                server_settings.port(),
                &error,
            ));
        }

        ui::window::open_initial(cx, first_startup, root, pending_notifications);
    });
}
