//! The local server: an OpenAPI-documented localhost HTTP API embedded in the app.

pub mod api;
pub mod error;
pub mod vaults;

use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex, PoisonError};
use std::thread::JoinHandle;

use poem::listener::TcpAcceptor;
use poem::middleware::Cors;
use poem::{EndpointExt, IntoResponse, Route, Server, get, http::Method};
use poem_openapi::OpenApiService;
use tokio::sync::Notify;

use crate::app::settings;
use api::{Api, ApiContext};

pub const BIND_HOST: &str = "127.0.0.1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerConfig {
    pub port: u16,
    pub token: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServerStatus {
    Running(u16),
    Stopped,
    Failed(String),
}

struct Running {
    config: ServerConfig,
    bound_port: u16,
    shutdown: Arc<Notify>,
    thread: Option<JoinHandle<()>>,
}

pub struct ServerManager {
    running: Option<Running>,
    /// Why the last startup attempt failed, for `status()` long after the `Err` was reported.
    last_error: Option<String>,
}

impl Default for ServerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerManager {
    pub const fn new() -> Self {
        Self {
            running: None,
            last_error: None,
        }
    }

    #[must_use]
    pub fn status(&self) -> ServerStatus {
        if let Some(running) = &self.running {
            return ServerStatus::Running(running.bound_port);
        }
        self.last_error
            .clone()
            .map_or(ServerStatus::Stopped, ServerStatus::Failed)
    }

    /// Starts, restarts, or stops the server so it matches `config` (`None` stops it).
    /// An `Err` return means the server could not bind;
    /// the manager keeps its previous state in that case.
    pub fn apply(&mut self, config: Option<ServerConfig>) -> Result<(), String> {
        let context = config
            .as_ref()
            .map(|config| ApiContext::new(config.token.clone(), Arc::new(configured_vaults)));
        self.apply_with_context(config, context)
    }

    /// [`ServerManager::apply`] with an explicit API context;
    /// tests use this to serve temporary vaults instead of the configured ones.
    pub fn apply_with_context(
        &mut self,
        config: Option<ServerConfig>,
        context: Option<ApiContext>,
    ) -> Result<(), String> {
        if let Some(running) = &self.running
            && Some(&running.config) == config.as_ref()
        {
            return Ok(());
        }
        self.last_error = None;
        self.stop();

        let Some(config) = config else {
            // No configuration means server shutdown
            return Ok(());
        };
        let Some(context) = context else {
            return Err("Missing API context".to_owned());
        };

        let listener = TcpListener::bind((BIND_HOST, config.port)).map_err(|error| {
            let message = format!(
                "Failed to bind local server on {BIND_HOST}:{}: {error}",
                config.port
            );
            self.last_error = Some(message.clone());
            message
        })?;
        let bound_port = listener
            .local_addr()
            .map_err(|error| {
                let message = format!("Failed to read local server port: {error}");
                self.last_error = Some(message.clone());
                message
            })?
            .port();
        listener.set_nonblocking(true).map_err(|error| {
            let message = format!("Failed to prepare local server socket: {error}");
            self.last_error = Some(message.clone());
            message
        })?;

        let shutdown = Arc::new(Notify::new());
        let thread_shutdown = Arc::clone(&shutdown);
        let thread = std::thread::Builder::new()
            .name("local-server".to_owned())
            .spawn(move || serve(listener, context, thread_shutdown))
            .map_err(|error| {
                let message = format!("Failed to spawn local server thread: {error}");
                self.last_error = Some(message.clone());
                message
            })?;

        self.running = Some(Running {
            config,
            bound_port,
            shutdown,
            thread: Some(thread),
        });
        Ok(())
    }

    /// Stops the server, if running. Safe to call repeatedly.
    pub fn stop(&mut self) {
        let Some(running) = self.running.take() else {
            return;
        };
        running.shutdown.notify_one();
        if let Some(thread) = running.thread {
            let _ = thread.join();
        }
    }
}

fn serve(listener: TcpListener, context: ApiContext, shutdown: Arc<Notify>) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("Failed to start local server runtime: {error}");
            return;
        }
    };
    let app = build_app(context, 0);
    runtime.block_on(async move {
        let acceptor = match TcpAcceptor::from_std(listener) {
            Ok(acceptor) => acceptor,
            Err(error) => {
                eprintln!("Failed to start local server listener: {error}");
                return;
            }
        };
        let _ = Server::new_with_acceptor(acceptor)
            .run_with_graceful_shutdown(
                app,
                shutdown.notified(),
                Some(std::time::Duration::from_secs(1)),
            )
            .await;
    });
}

/// Assembles the full application:
/// `OpenAPI` handlers, generated spec and UI, strict preflight, and the body-size guard.
fn build_app(context: ApiContext, port: u16) -> impl poem::Endpoint + 'static {
    let api_service = OpenApiService::new(
        Api::new(context),
        "Datalith Local Server",
        env!("CARGO_PKG_VERSION"),
    )
    .server(format!("http://{BIND_HOST}:{port}"));
    let spec = api_service.spec();
    let ui = api_service.swagger_ui();
    Route::new()
        .nest("/", api_service)
        .nest(
            "/openapi.json",
            get(poem::endpoint::make_sync(move |_| {
                spec.clone().into_response()
            })),
        )
        .nest("/docs", ui)
        .with(cors())
        .with(api::BodyLimit {
            max_size: api::MAX_BODY_BYTES,
        })
}

/// The preflight policy:
/// browser-extension origins only.
/// Non-browser clients carry no `Origin` header and pass through untouched;
/// plain webpages are refused (403) so an untokened server cannot be driven by drive-by scripts.
fn cors() -> Cors {
    Cors::new()
        .allow_origins_fn(|origin| {
            origin.starts_with("chrome-extension://") || origin.starts_with("moz-extension://")
        })
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(["authorization", "content-type"])
        .max_age(86_400)
}

/// The vaults clients may save into, last used first.
/// Directories that no longer exist are skipped.
fn configured_vaults() -> Vec<vaults::Vault> {
    let settings = settings::snapshot();
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Some(last) = &settings.last_vault {
        paths.push(last.clone());
    }
    for recent in &settings.recent_vaults {
        if !paths.contains(recent) {
            paths.push(recent.clone());
        }
    }
    paths
        .into_iter()
        .filter(|path| path.is_dir())
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
    manager_lock().apply(config)
}

#[must_use]
pub fn status() -> ServerStatus {
    manager_lock().status()
}

#[cfg(test)]
mod tests;
