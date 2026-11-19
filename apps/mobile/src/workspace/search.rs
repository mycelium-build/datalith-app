//! One search field for the vault. It searches the whole vault by default;
//! from a subfolder a visible segmented control limits it to that folder.
//! Names match from the first character, contents from three.
use super::Workspace;
use datalith_vault::search::{SearchHit, picker};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{App, AppContext as _, Context, Entity, Subscription, Task, Window};
use std::path::{Path, PathBuf};

pub const MIN_CONTENT_CHARS: usize = 3;
const RECENT_LIMIT: usize = 20;

/// The search field shared by every folder, its scope and its results.
pub struct SearchState {
    pub input: Entity<InputState>,
    pub task: Option<Task<()>>,
    pub results: Results,
    /// False searches the whole vault (the default); true limits the search
    /// to the folder on screen.
    pub folder_only: bool,
    /// While the empty field has focus, recent documents replace the folder.
    pub focused: bool,
    /// Absolute paths, most recently used first.
    pub recent: Vec<PathBuf>,
    _events: Subscription,
}

impl SearchState {
    pub fn new(window: &mut Window, cx: &mut Context<Workspace>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        let events = cx.subscribe(&input, |view: &mut Workspace, _, event, cx| match event {
            InputEvent::Change => view.search_changed(cx),
            InputEvent::Focus | InputEvent::Blur => {
                view.search.focused = matches!(event, InputEvent::Focus);
                cx.notify();
            }
            InputEvent::PressEnter { .. } => {}
        });
        Self {
            input,
            task: None,
            results: Results::default(),
            folder_only: false,
            focused: false,
            recent: Vec::new(),
            _events: events,
        }
    }

    pub fn query(&self, cx: &App) -> String {
        self.input.read(cx).value().to_string()
    }
}

#[derive(Default)]
pub struct Results {
    pub names: Vec<PathBuf>,
    pub contents: Vec<SearchHit>,
}

/// Files whose name contains `query`, open documents first, then by name.
pub fn names(
    paths: Vec<PathBuf>,
    open: &[PathBuf],
    query: &str,
    scope: Option<&Path>,
) -> Vec<PathBuf> {
    let entries = picker::collect_from_paths(
        paths
            .into_iter()
            .filter(|p| scope.is_none_or(|dir| p.starts_with(dir))),
    );
    if query.trim().is_empty() {
        return Vec::new();
    }
    picker::filter(&entries, open, query)
        .into_iter()
        .map(|entry| entry.path)
        .collect()
}

/// Content hits not already listed by name, inside `scope`.
pub fn contents(hits: Vec<SearchHit>, names: &[PathBuf], scope: Option<&Path>) -> Vec<SearchHit> {
    hits.into_iter()
        .filter(|hit| !names.contains(&hit.path))
        .filter(|hit| scope.is_none_or(|dir| hit.path.starts_with(dir)))
        .collect()
}

pub fn searches_contents(query: &str) -> bool {
    query.trim().chars().count() >= MIN_CONTENT_CHARS
}

/// Most recently used first, without duplicates.
pub fn remember(recent: &mut Vec<PathBuf>, path: PathBuf) {
    recent.retain(|p| *p != path);
    recent.insert(0, path);
    recent.truncate(RECENT_LIMIT);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn docs() -> Vec<PathBuf> {
        let root = crate::DOCS.root();
        crate::DOCS_FILES
            .iter()
            .map(|(name, _)| root.join(name))
            .collect()
    }

    #[test]
    fn the_vault_root_finds_a_nested_base_by_name() {
        let root = crate::DOCS.root();
        let found = names(docs(), &[], "Library", None);
        assert!(
            found.contains(&root.join("examples/bases/Library.base")),
            "{found:?}"
        );
    }

    #[test]
    fn folder_scope_keeps_only_that_folder() {
        let root = crate::DOCS.root();
        let scope = root.join("examples");
        let found = names(docs(), &[], "e", Some(&scope));
        assert_ne!(found, Vec::<PathBuf>::new());
        assert!(found.iter().all(|p| p.starts_with(&scope)));
    }

    #[test]
    fn open_documents_come_first() {
        let paths = vec![PathBuf::from("/v/a note.md"), PathBuf::from("/v/b note.md")];
        let open = [PathBuf::from("/v/b note.md")];
        assert_eq!(names(paths, &open, "note", None).first(), open.first());
    }

    #[test]
    fn content_hits_skip_name_matches_and_respect_scope() {
        let hit = |p: &str| SearchHit {
            path: p.into(),
            score: 1.0,
            snippet: "a word".into(),
            highlights: std::iter::once(2..6).collect(),
        };
        let hits = vec![hit("/v/a.md"), hit("/v/d/b.md"), hit("/v/c.md")];
        let kept = contents(hits.clone(), &[PathBuf::from("/v/a.md")], None);
        assert_eq!(kept.len(), 2);
        let scoped = contents(hits, &[], Some(Path::new("/v/d")));
        assert_eq!(scoped.len(), 1);
        assert!(!searches_contents("ab") && searches_contents("abc"));
    }

    #[test]
    fn recents_are_in_order_of_use() {
        let mut recent = Vec::new();
        for p in ["a", "b", "a", "c"] {
            remember(&mut recent, p.into());
        }
        assert_eq!(recent, [PathBuf::from("c"), "a".into(), "b".into()]);
    }
}

#[cfg(test)]
mod catalog_tests {
    use super::*;
    use datalith_vault::VaultCatalog;

    #[test]
    fn a_content_word_returns_a_highlighted_excerpt() -> anyhow::Result<()> {
        crate::test_environment();
        let vault = tempfile::tempdir()?;
        let note = vault.path().join("Garden.md");
        std::fs::write(&note, "# Garden\n\nThe persimmon tree ripens in autumn.\n")?;
        let catalog = VaultCatalog::open(
            vault.path(),
            datalith_core::file_types::registered_file_types(),
        )?;
        let start = std::time::Instant::now();
        while catalog.state() == datalith_vault::CatalogState::Syncing
            && start.elapsed() < std::time::Duration::from_secs(10)
        {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let names = names(catalog.paths(), &[], "persimmon", None);
        let hits = contents(catalog.search("persimmon"), &names, None);
        let hit = hits.iter().find(|h| h.path.ends_with("Garden.md"));
        anyhow::ensure!(hit.is_some(), "no content hit in {hits:?}");
        let hit = hit.ok_or_else(|| anyhow::anyhow!("missing hit"))?;
        anyhow::ensure!(hit.snippet.contains("persimmon") && !hit.highlights.is_empty());
        Ok(())
    }
}
