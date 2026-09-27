//! The documentation vault is read directly from the build-time resource bundle.

use std::path::PathBuf;

pub const DOCS_VAULT_NAME: &str = crate::vault::source::DOCUMENTATION.name();
pub const WELCOME_NOTE: &str = "Welcome.md";

pub fn docs_vault_path() -> PathBuf {
    crate::vault::source::DOCUMENTATION.root()
}
