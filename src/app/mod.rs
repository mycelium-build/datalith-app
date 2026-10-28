pub mod actions;
pub mod assets;
mod atomic_file;
pub mod deeplink;
pub mod docs;
pub mod fonts;
pub mod keymap;
mod machine;
pub mod menus;
pub mod preferences;
pub mod settings;
mod state;
pub mod system;
pub mod themes;
pub mod update;
pub mod version;
pub mod workspace;

use std::path::PathBuf;

pub use state::AppState;

pub fn init(cx: &mut gpui_kit::App) {
    gpui_kit::init(cx);
    cx.set_http_client(std::sync::Arc::new(reqwest_client::ReqwestClient::new()));
    update::init(cx);
}

pub fn data_dir() -> PathBuf {
    crate::channel::Channel::current().app_data_dir()
}
