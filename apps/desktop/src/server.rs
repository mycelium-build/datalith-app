use crate::app::settings;
pub use datalith_server::{BIND_HOST, ServerStatus};
use datalith_server::{ServerConfig, ServerManager, api::ApiContext, vaults};
use std::sync::{Arc, LazyLock, Mutex, PoisonError};
/// The vaults clients may save into, last used first.
fn configured_vaults() -> Vec<vaults::Vault> {
    settings::known_vault_paths()
        .into_iter()
        .map(|path| vaults::Vault {
            name: path.file_name().map_or_else(
                || path.to_string_lossy().into_owned(),
                |name| name.to_string_lossy().into_owned(),
            ),
            path,
        })
        .collect()
}

static MANAGER: LazyLock<Mutex<ServerManager>> = LazyLock::new(|| Mutex::new(ServerManager::new()));

fn manager_lock() -> std::sync::MutexGuard<'static, ServerManager> {
    MANAGER.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Reconciles the running server with the persisted settings.
/// Call after every server settings change and at startup.
pub fn sync() -> Result<(), String> {
    let server = settings::snapshot().server;
    let config = server.enabled().then_some(ServerConfig {
        port: server.port(),
        token: server.token().map(str::to_owned),
    });
    let context = config
        .as_ref()
        .map(|config| ApiContext::new(config.token.clone(), Arc::new(configured_vaults)));
    manager_lock().apply_with_context(config, context)
}

#[must_use]
pub fn status() -> ServerStatus {
    manager_lock().status()
}
