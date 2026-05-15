pub(crate) mod palette;
pub(crate) mod render;
pub(crate) mod sidebar;
pub(crate) mod tabs;

use std::path::{Path, PathBuf};
use std::time::Instant;

use gpui::*;
use gpui_component::{
    input::InputState,
    select::{SelectEvent, SelectItem, SelectState},
    tree::TreeState,
};

use crate::config::{add_recent_vault, load_recent_vaults, save_last_folder};
use crate::filetree::build_file_items;
use crate::search::SearchEngine;
use crate::utils::file_name_str;
use palette::Palette;

#[derive(Clone, Debug)]
pub(crate) enum VaultEntry {
    Vault {
        path: SharedString,
        name: SharedString,
    },
    OpenNew(SharedString),
}

impl SelectItem for VaultEntry {
    type Value = SharedString;

    fn title(&self) -> SharedString {
        match self {
            VaultEntry::Vault { name, .. } => name.clone(),
            VaultEntry::OpenNew(_) => SharedString::from("Open new vault..."),
        }
    }

    fn value(&self) -> &Self::Value {
        match self {
            VaultEntry::Vault { path, .. } => path,
            VaultEntry::OpenNew(marker) => marker,
        }
    }
}

pub(crate) struct OpenFile {
    pub(crate) path: PathBuf,
    pub(crate) state: Option<Entity<InputState>>,
    pub(crate) _sub: Option<Subscription>,
}

pub struct DatalithView {
    pub(crate) tree_state: Entity<TreeState>,
    pub(crate) vault_select_state: Entity<SelectState<Vec<VaultEntry>>>,
    pending_vault_refresh: bool,
    _vault_select_sub: Subscription,
    pub(crate) root_path: Option<PathBuf>,
    root_name: SharedString,
    pub(crate) open_files: Vec<OpenFile>,
    pub(crate) pending_open: Option<PathBuf>,
    pub(crate) active_tab: usize,
    pub(crate) search_engine: Option<SearchEngine>,
    pub(crate) palette: Palette,
    _palette_sub: Subscription,
    pub(crate) context_menu_target: Option<PathBuf>,
    pub(crate) rename_target: Option<PathBuf>,
    pub(crate) rename_state: Option<Entity<InputState>>,
    _rename_sub: Option<Subscription>,
    pub(crate) drag_hover: Option<(PathBuf, Instant)>,
    pub(crate) focus_sidebar_requested: bool,
    sidebar_focus_handle: FocusHandle,
    _sidebar_blur_sub: Option<Subscription>,
}

impl DatalithView {
    #[must_use]
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let palette = Palette::new(window, cx);
        let palette_sub = Palette::input_subscription(palette.input.clone(), window, cx);
        let sidebar_focus_handle = cx.focus_handle();
        let tree_state = cx.new(|cx| TreeState::new(cx));

        let recent_vaults: Vec<VaultEntry> = load_recent_vaults()
            .into_iter()
            .map(|p| {
                let path: SharedString = p.to_string_lossy().to_string().into();
                let name: SharedString = file_name_str(&p).into();
                VaultEntry::Vault { path, name }
            })
            .collect();
        let mut items = recent_vaults;
        items.push(VaultEntry::OpenNew(SharedString::from(
            crate::consts::VAULT_SELECT_MARKER,
        )));

        let vault_select_state = cx.new(|cx| SelectState::new(items, None, window, cx));

        let vault_select_sub = cx.subscribe_in(
            &vault_select_state,
            window,
            |view: &mut DatalithView, _state, event: &SelectEvent<Vec<VaultEntry>>, window, cx| {
                match event {
                    SelectEvent::Confirm(value) => {
                        if let Some(value) = value {
                            if value == crate::consts::VAULT_SELECT_MARKER {
                                window.dispatch_action(Box::new(crate::actions::OpenVault), cx);
                            } else {
                                let path = PathBuf::from(value.to_string());
                                view.set_root_path(path, cx);
                            }
                        }
                    }
                }
            },
        );

        let sidebar_blur_sub = cx.on_blur(&sidebar_focus_handle, window, {
            let tree_state = tree_state.clone();
            move |_this, _window, cx| {
                tree_state.update(cx, |state, cx| {
                    state.set_selected_index(None, cx);
                });
            }
        });

        Self {
            tree_state,
            vault_select_state,
            root_path: None,
            root_name: "No folder open".into(),
            open_files: Vec::new(),
            active_tab: 0,
            search_engine: None,
            palette,
            _palette_sub: palette_sub,
            _rename_sub: None,
            _sidebar_blur_sub: Some(sidebar_blur_sub),
            _vault_select_sub: vault_select_sub,
            context_menu_target: None,
            rename_target: None,
            rename_state: None,
            drag_hover: None,
            focus_sidebar_requested: false,
            pending_open: None,
            pending_vault_refresh: false,
            sidebar_focus_handle,
        }
    }

    pub(crate) fn set_root_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.root_name = file_name_str(&path).into();
        self.root_path = Some(path.clone());
        let _ = save_last_folder(&path);
        let _ = add_recent_vault(&path);

        self.pending_vault_refresh = true;

        let items = build_file_items(&path);
        self.tree_state.update(cx, |state, cx| {
            state.set_items(items, cx);
        });

        self.search_engine = match SearchEngine::new(&path) {
            Ok(engine) => Some(engine),
            Err(e) => {
                eprintln!("Failed to build search index: {e}");
                None
            }
        };

        self.palette.set_root(self.search_engine.as_ref());

        cx.notify();
    }

    pub(crate) fn refresh_vault_select(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let recent_vaults: Vec<VaultEntry> = load_recent_vaults()
            .into_iter()
            .map(|p| {
                let path: SharedString = p.to_string_lossy().to_string().into();
                let name: SharedString = file_name_str(&p).into();
                VaultEntry::Vault { path, name }
            })
            .collect();
        let mut items = recent_vaults;
        items.push(VaultEntry::OpenNew(SharedString::from(
            crate::consts::VAULT_SELECT_MARKER,
        )));
        self.vault_select_state
            .update(cx, |state, cx| state.set_items(items, window, cx));
        self.pending_vault_refresh = false;
    }

    #[must_use]
    pub(crate) fn resolve_target(&self, cx: &Context<Self>) -> Option<PathBuf> {
        self.tree_state
            .read(cx)
            .selected_entry()
            .map(|e| PathBuf::from(e.item().id.to_string()))
            .or_else(|| {
                let active = self.active_tab.min(self.open_files.len().saturating_sub(1));
                self.open_files.get(active).map(|f| f.path.clone())
            })
    }

    pub(crate) fn refresh_tree(&mut self, cx: &mut Context<Self>) {
        if let Some(ref root) = self.root_path {
            let expanded_ids = self.tree_state.read(cx).expanded_ids();
            let mut items = build_file_items(root);
            for item in &mut items {
                if expanded_ids.contains(&item.id) {
                    item.set_expanded(true);
                }
            }
            self.tree_state.update(cx, |state, cx| {
                state.set_items(items, cx);
            });
            cx.notify();
        }
    }

    pub(crate) fn track_new_file(&mut self, path: &Path) {
        if let Some(ref engine) = self.search_engine {
            let _ = engine.indexer.add_file(path);
        }
        self.palette.add_entry(path);
    }

    pub(crate) fn track_file_rename(&mut self, old_path: &Path, new_path: &Path) {
        if let Some(ref engine) = self.search_engine {
            let _ = engine.indexer.rename_file(old_path, new_path);
        }
        self.palette.rename_entry(old_path, new_path);
    }

    fn remove_indexed_under(&mut self, root: &Path) {
        if let Some(ref engine) = self.search_engine {
            let prefix = root.to_string_lossy().to_string();
            for path in engine.indexer.all_paths() {
                if path.to_string_lossy().starts_with(&prefix) {
                    let _ = engine.indexer.remove_file(&path);
                    self.palette.remove_entry(&path);
                }
            }
        }
    }

    pub(crate) fn track_file_delete(&mut self, path: &Path) {
        if path.is_dir() {
            self.remove_indexed_under(path);
        } else {
            if let Some(ref engine) = self.search_engine {
                let _ = engine.indexer.remove_file(path);
            }
            self.palette.remove_entry(path);
        }
    }

    pub(crate) fn track_file_edited(&mut self, path: &Path) {
        if let Some(ref engine) = self.search_engine {
            let _ = engine.indexer.add_file(path);
        }
    }
}
