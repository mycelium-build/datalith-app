//! Offline touch workspace shared by the two native hosts.
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::Context as _;
use datalith_ui::{assets::DatalithAssets, fonts::ResolvedFonts};
use datalith_vault::environment::{self, VaultEnvironment};
use datalith_vault::source::EmbeddedVault;
use gpui_kit::component::{ActiveTheme, Root, Theme, ThemeMode, theme::ThemeSet};
use gpui_kit::{AppContext, Application, WindowOptions};

mod browser;
mod commands;
use datalith_vault::draft;
#[cfg(target_os = "ios")]
mod ios;
#[cfg(target_os = "ios")]
mod ios_folder;
#[cfg(target_os = "ios")]
mod ios_provider;
#[cfg(target_os = "ios")]
use ios_folder as external_folder;
#[cfg(target_os = "android")]
mod android_folder;
#[cfg(target_os = "android")]
use android_folder as external_folder;

#[cfg(target_os = "ios")]
use datalith_ui::ios_input;
mod workspace;

include!(concat!(env!("OUT_DIR"), "/docs_vault.rs"));
const DOCS: EmbeddedVault =
    EmbeddedVault::new("documentation", "Datalith Documentation", DOCS_FILES);
static DATA: OnceLock<PathBuf> = OnceLock::new();

/// Opens the app-private vault, copying the starter files on first launch.
///
/// # Errors
/// Returns the filesystem error when the documents directory cannot be seeded.
pub fn starter(documents: &Path) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(documents)?;
    let root = documents.join("Datalith");
    if !root.is_dir() {
        let stage = tempfile::tempdir_in(documents)?;
        for (name, bytes) in DOCS_FILES {
            let file = stage.path().join(name);
            std::fs::create_dir_all(file.parent().context("Missing parent directory")?)?;
            std::fs::write(file, bytes)?;
        }
        std::fs::rename(stage.path(), &root)?;
    }
    Ok(root)
}

#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub extern "C" fn datalith_ios_main() {
    if let Some(documents) = dirs::document_dir() {
        launch(gpui_platform::application(), documents);
    }
}

pub fn run_android(app: Application, files: &Path) {
    launch(app, files.join("Documents"));
}

fn launch(app: Application, documents: PathBuf) {
    #[cfg(target_os = "ios")]
    let private = dirs::data_local_dir()
        .unwrap_or_else(|| documents.clone())
        .join("Datalith");
    #[cfg(not(target_os = "ios"))]
    let private = documents.parent().unwrap_or(&documents).join("workspace");
    if let Err(e) = std::fs::create_dir_all(&private) {
        eprintln!("Cannot prepare mobile storage: {e}");
        return;
    }
    // Preserve settings from earlier mobile builds.
    let preferences = private.join(".mobile-preferences.json");
    if !preferences.exists() {
        let _ = std::fs::rename(documents.join(".mobile-preferences.json"), &preferences);
    }
    DATA.get_or_init(|| private);
    environment::init(VaultEnvironment {
        embedded_vaults: &[DOCS],
        channel_directory: None,
        app_data_dir: || DATA.get().cloned().unwrap_or_default(),
    });
    #[cfg(target_os = "ios")]
    app.on_open_urls(|urls| {
        for url in urls {
            commands::link(url);
        }
    });
    app.with_assets(DatalithAssets).run(move |cx| {
        gpui_kit::init(cx);
        let theme = include_str!("../../../assets/themes/datalith.json");
        match serde_json::from_str::<ThemeSet>(theme) {
            Ok(set) => {
                for config in set.themes {
                    match config.mode {
                        ThemeMode::Light => Theme::global_mut(cx).light_theme = config.into(),
                        ThemeMode::Dark => Theme::global_mut(cx).dark_theme = config.into(),
                    }
                }
                Theme::sync_system_appearance(None, cx);
            }
            Err(error) => eprintln!("Cannot load Datalith theme: {error}"),
        }
        let headings = match datalith_ui::fonts::load_pixeloid(cx) {
            Ok(()) => datalith_ui::fonts::PIXELOID_FONT.into(),
            Err(_) => cx.theme().font_family.clone(),
        };
        cx.set_global(ResolvedFonts {
            interface: cx.theme().font_family.clone(),
            headings,
            reading: cx.theme().font_family.clone(),
            code: cx.theme().mono_font_family.clone(),
        });
        workspace::FontCatalog::install(cx);
        if let Err(error) = cx.open_window(WindowOptions::default(), |window, cx| {
            let view = cx.new(|cx| workspace::Workspace::new(starter(&documents), window, cx));
            commands::attach(&view, window, cx);
            #[cfg(target_os = "ios")]
            ios::attach(&view, window, cx);
            cx.new(|cx| Root::new(view, window, cx))
        }) {
            eprintln!("Cannot open the mobile workspace: {error}");
        }
    });
}

/// Initializes the vault environment once for tests, as `launch` does.
#[cfg(test)]
pub(crate) fn test_environment() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        let data =
            std::env::temp_dir().join(format!("datalith-mobile-tests-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&data);
        DATA.get_or_init(|| data);
        environment::init(VaultEnvironment {
            embedded_vaults: &[DOCS],
            channel_directory: None,
            app_data_dir: || DATA.get().cloned().unwrap_or_default(),
        });
    });
}
