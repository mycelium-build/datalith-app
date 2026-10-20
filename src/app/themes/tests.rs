use std::fs;

use super::*;

struct Sandbox {
    root: PathBuf,
}
impl Sandbox {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "datalith-theme-domain-{:032x}",
            rand::random::<u128>()
        ));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }
    fn library(&self) -> ThemeLibrary {
        settings::with_store(|store| {
            *store = settings::SettingsStore::new(self.root.join("preferences.json"));
        });
        let mut library = ThemeLibrary::new(self.root.join("themes"));
        for set in embedded::sets() {
            library.insert_set(set, ThemeSource::Bundled).unwrap();
        }
        library
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn current_document_resolves_selection_and_falls_back_by_mode() {
    let sandbox = Sandbox::new();
    let mut library = sandbox.library();
    let variant = library.variant("Catppuccin Latte").unwrap();
    let id = variant.id();
    library.set_current(id, ThemeKind::Light).unwrap();
    assert_eq!(
        library.current_document(ThemeKind::Light).name(),
        "Catppuccin Latte"
    );

    library.slots = ["Missing light".into(), "Missing dark".into()];
    for kind in [ThemeKind::Light, ThemeKind::Dark] {
        let document = library.current_document(kind);
        assert_eq!(document.mode(), kind.mode());
        assert_eq!(
            document.name(),
            match kind {
                ThemeKind::Light => DEFAULT_LIGHT,
                ThemeKind::Dark => DEFAULT_DARK,
            }
        );
    }
    assert_eq!(library.current(ThemeKind::Light), "Missing light");
    assert_eq!(library.current(ThemeKind::Dark), "Missing dark");
}
