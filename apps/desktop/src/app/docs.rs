//! The documentation vault is read directly from the build-time resource bundle.

use std::path::PathBuf;

use datalith_vault::{environment, source::EmbeddedVault};
include!(concat!(env!("OUT_DIR"), "/docs_vault.rs"));
pub const DOCUMENTATION: EmbeddedVault =
    EmbeddedVault::new("documentation", "Datalith Documentation", DOCS_FILES);

pub fn init_environment() {
    let channel = crate::channel::Channel::current();
    environment::init(environment::VaultEnvironment {
        embedded_vaults: &[DOCUMENTATION],
        channel_directory: (channel != crate::channel::Channel::Stable).then(|| channel.name()),
        app_data_dir: super::data_dir,
    });
}

pub const DOCS_VAULT_NAME: &str = DOCUMENTATION.name();
pub const WELCOME_NOTE: &str = "Welcome.md";

pub fn docs_vault_path() -> PathBuf {
    DOCUMENTATION.root()
}
