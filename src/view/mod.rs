pub(crate) mod markdown_editor;
pub(crate) mod palette;
pub(crate) mod render;
pub(crate) mod settings;
pub(crate) mod sidebar;
pub(crate) mod tabs;

use std::path::{Path, PathBuf};
use std::time::Instant;

use gpui::*;
use gpui_component::{
    input::InputState,
    select::{SelectEvent, SelectItem, SelectState},
    slider::SliderEvent,
    tree::TreeState,
};

use crate::config::{add_recent_vault, load_recent_vaults, save_last_folder};
use crate::link_cache::LinkCache;
use crate::search::SearchEngine;
use crate::utils::file_name_str;
use crate::view::markdown_editor::MarkdownEditor;
use crate::view::sidebar::file_tree::build_file_items;
use palette::Palette;
use settings::SettingsView;

#[derive(Clone, Copy, Debug)]
pub(crate) enum NavigationAction {
    GoBack,
    GoForward,
}

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
    pub(crate) markdown_editor: Option<Entity<MarkdownEditor>>,
    pub(crate) _sub: Option<Subscription>,
    pub(crate) _md_sub: Option<Subscription>,
    pub(crate) editor_mode: bool,
    pub(crate) navigation_stack: Vec<PathBuf>,
    pub(crate) navigation_position: usize,
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
    pub(crate) link_cache: Option<LinkCache>,
    pub(crate) palette: Palette,
    _palette_sub: Subscription,
    pub(crate) settings: SettingsView,
    _font_size_slider_sub: Subscription,
    pub(crate) context_menu_target: Option<PathBuf>,
    pub(crate) rename_target: Option<PathBuf>,
    pub(crate) rename_state: Option<Entity<InputState>>,
    _rename_sub: Option<Subscription>,
    pub(crate) drag_hover: Option<(PathBuf, Instant)>,
    pub(crate) focus_sidebar_requested: bool,
    pub(crate) focus_editor_requested: bool,
    sidebar_focus_handle: FocusHandle,
    pub(crate) last_sidebar_selection: Option<PathBuf>,
    pub(crate) pending_navigation: Option<NavigationAction>,
    in_navigation: bool,
}

fn build_vault_entries() -> Vec<VaultEntry> {
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
    items
}

impl DatalithView {
    #[must_use]
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let palette = Palette::new(window, cx);
        let palette_sub = Palette::input_subscription(palette.input.clone(), window, cx);
        let sidebar_focus_handle = cx.focus_handle();
        let tree_state = cx.new(|cx| TreeState::new(cx));

        let vault_select_state =
            cx.new(|cx| SelectState::new(build_vault_entries(), None, window, cx));

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

        let settings = SettingsView::new(cx);
        let font_size_slider_sub = cx.subscribe(
            &settings.font_size_slider_state,
            |_view, _, event: &SliderEvent, cx| {
                let SliderEvent::Change(value) = event;
                let val = value.start() as f64;
                let new_size = px(crate::consts::BASE_FONT_SIZE as f32 * value.start());
                cx.global_mut::<settings::ThemeOptions>()
                    .font_size_multiplier = val;
                gpui_component::Theme::global_mut(cx).font_size = new_size;
                cx.refresh_windows();
                let _ = crate::config::save_font_size_multiplier(val);
            },
        );

        Self {
            tree_state,
            vault_select_state,
            root_path: None,
            root_name: "No folder open".into(),
            open_files: Vec::new(),
            active_tab: 0,
            search_engine: None,
            link_cache: None,
            palette,
            _palette_sub: palette_sub,
            settings,
            _font_size_slider_sub: font_size_slider_sub,
            _rename_sub: None,
            _vault_select_sub: vault_select_sub,
            context_menu_target: None,
            rename_target: None,
            rename_state: None,
            drag_hover: None,
            focus_sidebar_requested: false,
            focus_editor_requested: false,
            pending_open: None,
            pending_vault_refresh: false,
            sidebar_focus_handle,
            last_sidebar_selection: None,
            pending_navigation: None,
            in_navigation: false,
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

        self.link_cache = Some(LinkCache::new(&path));

        self.palette.set_root(self.search_engine.as_ref());

        cx.notify();
    }

    pub(crate) fn refresh_vault_select(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.vault_select_state.update(cx, |state, cx| {
            state.set_items(build_vault_entries(), window, cx)
        });
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
            .or_else(|| self.last_sidebar_selection.clone())
    }

    pub(crate) fn refresh_tree(&mut self, cx: &mut Context<Self>) {
        if let Some(ref root) = self.root_path {
            let expanded_ids = self.tree_state.read(cx).expanded_ids().clone();
            let items = build_file_items(root);
            self.tree_state.update(cx, |state, cx| {
                state.set_items(items, cx);
                for id in &expanded_ids {
                    state.expand_by_id(id, cx);
                }
            });
            cx.notify();
        }
    }

    pub(crate) fn track_new_file(&mut self, path: &Path) {
        if let Some(ref engine) = self.search_engine {
            let _ = engine.indexer.add_file(path);
        }
        if let Some(ref mut cache) = self.link_cache {
            cache.add_file(path);
        }
        self.palette.add_entry(path);
    }

    pub(crate) fn track_file_rename(&mut self, old_path: &Path, new_path: &Path) {
        if let Some(ref engine) = self.search_engine {
            let _ = engine.indexer.rename_file(old_path, new_path);
        }
        if let Some(ref mut cache) = self.link_cache {
            cache.rename_file(old_path, new_path);
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
        if let Some(ref mut cache) = self.link_cache {
            cache.remove_under(root);
        }
    }

    pub(crate) fn track_file_delete(&mut self, path: &Path) {
        if path.is_dir() {
            self.remove_indexed_under(path);
        } else {
            if let Some(ref engine) = self.search_engine {
                let _ = engine.indexer.remove_file(path);
            }
            if let Some(ref mut cache) = self.link_cache {
                cache.remove_file(path);
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
