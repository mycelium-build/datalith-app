//! Process-wide settings the application gives the vault once, at startup.
//!
//! The vault does not know which application or release channel runs it. The application
//! declares the read-only vaults it embeds and where per-channel caches live, by calling
//! [`init`] before opening a vault.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context as _, Result};

use crate::source::{self, EmbeddedVault};

/// What the application provides to every vault it opens.
#[derive(Clone, Copy)]
pub struct VaultEnvironment {
    /// Read-only vaults embedded in the application binary, such as its documentation.
    pub embedded_vaults: &'static [EmbeddedVault],
    /// Subdirectory of `.datalith` that keeps this channel's state apart from other channels,
    /// or `None` to use `.datalith` itself.
    pub channel_directory: Option<&'static str>,
    /// Returns the application data directory, which holds the caches of embedded vaults.
    pub app_data_dir: fn() -> PathBuf,
}

static ENVIRONMENT: OnceLock<VaultEnvironment> = OnceLock::new();

/// Installs the application's environment on first use. Subsequent calls keep that environment.
pub fn init(environment: VaultEnvironment) {
    ENVIRONMENT.get_or_init(|| environment);
}

fn current() -> Option<&'static VaultEnvironment> {
    #[cfg(any(test, feature = "test-support"))]
    ENVIRONMENT.get_or_init(crate::testing::environment);
    ENVIRONMENT.get()
}

/// The embedded vaults the application declared, or none before [`init`].
pub(crate) fn embedded_vaults() -> &'static [EmbeddedVault] {
    current().map_or(&[], |environment| environment.embedded_vaults)
}

/// Directory holding the catalog, search index and other state of the vault at `root`.
///
/// # Errors
///
/// Returns an error before [`init`], and for a path in the embedded namespace that matches no
/// embedded vault.
pub fn vault_dir(root: &Path) -> Result<PathBuf> {
    let environment = current().context("The vault environment is not initialized")?;
    vault_dir_with(
        root,
        environment.channel_directory,
        &(environment.app_data_dir)(),
    )
}

/// Directory holding the state of the vault at `root` for a given channel layout.
///
/// Directory vaults keep it in `.datalith`, or in `.datalith/{channel_directory}`. Embedded
/// vaults cannot be written to, so theirs lives in
/// `{app_data_dir}/immutable-vault-caches/{vault id}`.
///
/// # Errors
///
/// Returns an error for a path in the embedded namespace that is not the root of a declared
/// embedded vault.
pub fn vault_dir_with(
    root: &Path,
    channel_directory: Option<&str>,
    app_data_dir: &Path,
) -> Result<PathBuf> {
    if source::is_provider(root) {
        return Ok(app_data_dir
            .join("provider-caches")
            .join(root.file_name().context("Missing folder grant")?));
    }
    if source::is_read_only(root) {
        let vault = source::embedded_vault(root)
            .filter(|vault| vault.root() == root)
            .context("Unknown immutable vault cache")?;
        return Ok(app_data_dir.join("immutable-vault-caches").join(vault.id()));
    }
    let directory = root.join(crate::DATALITH_DIR_NAME);
    Ok(match channel_directory {
        None => directory,
        Some(channel) => directory.join(channel),
    })
}

pub(crate) fn app_data_dir() -> Result<PathBuf> {
    Ok((current()
        .context("Vault environment unavailable")?
        .app_data_dir)())
}
