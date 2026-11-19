//! Offline touch workspace shared by the two native hosts.
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::Context as _;
use datalith_ui::{assets::DatalithAssets, fonts::ResolvedFonts};
use datalith_vault::environment::{self, VaultEnvironment};
use datalith_vault::source::EmbeddedVault;
use gpui_kit::component::{ActiveTheme, Root};
use gpui_kit::{AppContext, Application, WindowOptions};

#[cfg(target_os = "ios")]
mod ios;
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
        launch(gpui_platform::application(), documents, None);
    }
}

pub fn run_android(app: Application, files: &Path, selected: Option<PathBuf>) {
    launch(app, files.join("Documents"), selected);
}

fn launch(app: Application, documents: PathBuf, selected: Option<PathBuf>) {
    DATA.get_or_init(|| documents.clone());
    environment::init(VaultEnvironment {
        embedded_vaults: &[DOCS],
        channel_directory: None,
        app_data_dir: || DATA.get().cloned().unwrap_or_default(),
    });
    app.with_assets(DatalithAssets).run(move |cx| {
        gpui_kit::init(cx);
        cx.set_global(ResolvedFonts {
            interface: cx.theme().font_family.clone(),
            headings: cx.theme().font_family.clone(),
            reading: cx.theme().font_family.clone(),
            code: cx.theme().mono_font_family.clone(),
        });
        if let Err(error) = cx.open_window(WindowOptions::default(), |window, cx| {
            let view =
                cx.new(|cx| workspace::Workspace::new(starter(&documents), selected, window, cx));
            #[cfg(target_os = "ios")]
            ios::attach(&view, window, cx);
            cx.new(|cx| Root::new(view, window, cx))
        }) {
            eprintln!("Cannot open the mobile workspace: {error}");
        }
    });
}
