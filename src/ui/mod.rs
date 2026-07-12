pub(crate) mod editors;
pub(crate) mod palette;
pub(crate) mod render;
pub(crate) mod settings;
pub(crate) mod sidebar;
pub(crate) mod tabs;
pub(crate) mod themes;
pub(crate) mod viewers;

pub(crate) const BASE_FONT_SIZE: f64 = 16.0;
const VAULT_SELECT_MARKER: &str = "__open_new__";

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gpui::*;
use gpui_component::{
    input::InputState,
    select::{SelectEvent, SelectItem, SelectState},
    slider::SliderEvent,
    tree::{TreeItem, TreeState},
};

use crate::app::config::{add_recent_vault, load_recent_vaults, save_last_folder};
use crate::document::handler::FileHandler;
use crate::document::registry::{self, FileRegistry};
use crate::ui::sidebar::file_tree::build_file_items_with_expanded;
use crate::vault::path::display_name;
use crate::vault::{CatalogUpdate, VaultCatalog};
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
    pub(crate) handler: Entity<FileHandler>,
    pub(crate) _sub: Option<Subscription>,
    pub(crate) _event_sub: Option<Subscription>,
    pub(crate) navigation_stack: Vec<PathBuf>,
    pub(crate) navigation_position: usize,
}

pub(crate) struct DatalithView {
    pub(crate) tree_state: Entity<TreeState>,
    pub(crate) vault_select_state: Entity<SelectState<Vec<VaultEntry>>>,
    pending_vault_refresh: bool,
    _vault_select_sub: Subscription,
    pub(crate) root_path: Option<PathBuf>,
    root_name: SharedString,
    pub(crate) open_files: Vec<OpenFile>,
    pub(crate) pending_open: Option<PathBuf>,
    pub(crate) active_tab: usize,
    pub(crate) vault_catalog: Option<VaultCatalog>,
    pub(crate) catalog_updates: Option<std::sync::mpsc::Receiver<CatalogUpdate>>,
    catalog_poll_task: Task<()>,
    pub(crate) pending_external_updates: Vec<PathBuf>,
    pub(crate) palette: Palette,
    _palette_sub: Subscription,
    pub(crate) settings: SettingsView,
    _font_size_slider_sub: Subscription,
    pub(crate) context_menu_target: Option<PathBuf>,
    pub(crate) suppress_sidebar_context_menu: bool,
    pub(crate) rename_target: Option<PathBuf>,
    pub(crate) rename_state: Option<Entity<InputState>>,
    _rename_sub: Option<Subscription>,
    pub(crate) drag_hover: Option<(PathBuf, Instant)>,
    pub(crate) expanded_tree_ids: Vec<SharedString>,
    pub(crate) focus_sidebar_requested: bool,
    pub(crate) focus_editor_requested: bool,
    sidebar_focus_handle: FocusHandle,
    pub(crate) last_sidebar_selection: Option<PathBuf>,
    pub(crate) pending_navigation: Option<NavigationAction>,
    in_navigation: bool,
    pub(crate) registry: FileRegistry,
}

fn build_vault_entries() -> Vec<VaultEntry> {
    let recent_vaults: Vec<VaultEntry> = load_recent_vaults()
        .into_iter()
        .map(|p| {
            let path: SharedString = p.to_string_lossy().to_string().into();
            let name: SharedString = display_name(&p).into();
            VaultEntry::Vault { path, name }
        })
        .collect();
    let mut items = recent_vaults;
    items.push(VaultEntry::OpenNew(SharedString::from(VAULT_SELECT_MARKER)));
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
                            if value == VAULT_SELECT_MARKER {
                                window
                                    .dispatch_action(Box::new(crate::app::actions::OpenVault), cx);
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
                let SliderEvent::Change(value) = event else {
                    return;
                };
                let val = value.start() as f64;
                let new_size = px(BASE_FONT_SIZE as f32 * value.start());
                cx.global_mut::<settings::ThemeOptions>()
                    .font_size_multiplier = val;
                gpui_component::Theme::global_mut(cx).font_size = new_size;
                cx.refresh_windows();
                let _ = crate::app::config::save_font_size_multiplier(val);
            },
        );

        Self {
            tree_state,
            vault_select_state,
            root_path: None,
            root_name: "No folder open".into(),
            open_files: Vec::new(),
            active_tab: 0,
            vault_catalog: None,
            catalog_updates: None,
            catalog_poll_task: Task::ready(()),
            pending_external_updates: Vec::new(),
            palette,
            _palette_sub: palette_sub,
            settings,
            _font_size_slider_sub: font_size_slider_sub,
            _rename_sub: None,
            _vault_select_sub: vault_select_sub,
            context_menu_target: None,
            suppress_sidebar_context_menu: false,
            rename_target: None,
            rename_state: None,
            drag_hover: None,
            expanded_tree_ids: Vec::new(),
            focus_sidebar_requested: false,
            focus_editor_requested: false,
            pending_open: None,
            pending_vault_refresh: false,
            sidebar_focus_handle,
            last_sidebar_selection: None,
            pending_navigation: None,
            in_navigation: false,
            registry: registry::default_registry(),
        }
    }

    pub(crate) fn set_root_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.root_name = display_name(&path).into();
        self.root_path = Some(path.clone());
        let _ = save_last_folder(&path);
        let _ = add_recent_vault(&path);

        self.pending_vault_refresh = true;

        self.expanded_tree_ids.clear();
        let items = build_file_items_with_expanded(&path, &self.expanded_tree_ids);
        self.tree_state.update(cx, |state, cx| {
            state.set_items(items, cx);
        });

        let file_types = self.registry.registered_file_types();
        self.vault_catalog = match VaultCatalog::open(path.clone(), file_types) {
            Ok(catalog) => Some(catalog),
            Err(e) => {
                eprintln!("Failed to open Vault Catalog: {e}");
                None
            }
        };
        self.catalog_updates = self.vault_catalog.as_ref().map(VaultCatalog::subscribe);
        self.palette.set_root(self.vault_catalog.as_ref());
        self.catalog_poll_task = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(200))
                    .await;
                if this
                    .update(cx, |view, cx| {
                        let mut changed_paths = Vec::new();
                        let mut structure_changed = false;
                        let mut tracked_paths_changed = false;
                        if let Some(ref updates) = view.catalog_updates {
                            while let Ok(update) = updates.try_recv() {
                                structure_changed |= update.structure_changed;
                                tracked_paths_changed |= update.tracked_paths_changed;
                                changed_paths.extend(update.changed_paths.iter().cloned());
                            }
                        }
                        if !changed_paths.is_empty() {
                            for removed in changed_paths.iter().filter(|path| !path.exists()) {
                                view.close_tabs_under(removed, cx);
                            }
                            view.pending_external_updates.extend(changed_paths);
                        }
                        if structure_changed {
                            view.refresh_tree(cx);
                        }
                        if tracked_paths_changed {
                            view.palette.set_root(view.vault_catalog.as_ref());
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        });

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
            let selected = self
                .tree_state
                .read(cx)
                .selected_entry()
                .map(|e| (e.item().id.clone(), e.item().label.clone()));
            let items = build_file_items_with_expanded(root, &self.expanded_tree_ids);
            self.tree_state.update(cx, |state, cx| {
                state.set_items(items, cx);
                if let Some((item_id, item_label)) = selected {
                    let item = TreeItem::new(item_id, item_label);
                    state.set_selected_item(Some(&item), cx);
                }
            });
            cx.notify();
        }
    }

    pub(crate) fn mark_tree_item_expanded(&mut self, id: &SharedString, expanded: bool) {
        if expanded {
            if !self
                .expanded_tree_ids
                .iter()
                .any(|expanded_id| expanded_id == id)
            {
                self.expanded_tree_ids.push(id.clone());
            }
        } else {
            self.expanded_tree_ids
                .retain(|expanded_id| expanded_id != id);
        }
    }

    pub(crate) fn expand_tree_item(&mut self, id: &SharedString, cx: &mut Context<Self>) {
        self.mark_tree_item_expanded(id, true);
        self.refresh_tree(cx);
    }

    pub(crate) fn visible_tree_entry_count(&self) -> usize {
        fn count_items(items: &[TreeItem]) -> usize {
            items
                .iter()
                .map(|item| {
                    1 + if item.is_expanded() {
                        count_items(&item.children)
                    } else {
                        0
                    }
                })
                .sum()
        }

        self.root_path
            .as_ref()
            .map(|root| {
                count_items(&build_file_items_with_expanded(
                    root,
                    &self.expanded_tree_ids,
                ))
            })
            .unwrap_or(0)
    }

    pub(crate) fn track_new_file(&mut self, path: &Path) {
        self.palette.add_entry(path);
    }
}
