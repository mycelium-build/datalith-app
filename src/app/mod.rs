pub mod actions;
pub mod assets;
pub mod docs;
pub mod fonts;
pub mod keymap;
pub mod menus;
pub mod preferences;
pub mod settings;
mod state;
pub mod system;

use std::path::PathBuf;

pub use state::AppState;

pub fn data_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_default().join("datalith")
}
