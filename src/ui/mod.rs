pub mod editors;
pub mod icons;
pub mod monolith;
pub mod notifications;
pub mod palette;
pub mod render;
pub mod settings;
pub mod sidebar;
pub mod startup;
pub mod tabs;
pub mod themes;
pub mod viewers;
pub mod window;

pub const BASE_FONT_SIZE: f32 = 16.0;
const LINE_HEIGHT: f32 = 1.6;
const VAULT_SELECT_MARKER: &str = "__open_new__";

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gpui::{
    AppContext, Context, Entity, FocusHandle, SharedString, Subscription, Task, Window, px,
};
use gpui_component::{
    input::InputState,
    notification::Notification,
    select::{SelectEvent, SelectItem, SelectState},
    slider::SliderEvent,
    tree::{TreeItem, TreeState},
};

use crate::app::settings as app_settings;
use crate::document::registry::{self, FileRegistry};
use crate::ui::sidebar::file_tree::build_file_items_with_expanded;
use crate::ui::startup::{StartupAnimation, StartupType};
use crate::vault::path::display_name;
use crate::vault::{CatalogEvent, CatalogState, VaultCatalog};
use palette::Palette;
use settings::SettingsView;

#[derive(Clone, Debug)]
pub enum VaultEntry {
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
            Self::Vault { name, .. } => name.clone(),
            Self::OpenNew(_) => SharedString::from("Open new vault..."),
        }
    }

    fn value(&self) -> &Self::Value {
        match self {
            Self::Vault { path, .. } => path,
            Self::OpenNew(marker) => marker,
        }
    }
}

// The view tracks several independent one-shot UI flags (focus requests, refresh notifications) that are read and cleared during rendering;
// grouping them would obscure the render loop's intent.
#[allow(clippy::struct_excessive_bools)]
pub struct DatalithView {
    pub(crate) tree_state: Entity<TreeState>,
    pub(crate) vault_select_state: Entity<SelectState<Vec<VaultEntry>>>,
    pending_vault_refresh: bool,
    _vault_select_sub: Subscription,
    pub(crate) root_path: Option<PathBuf>,
    root_name: SharedString,
    pub(crate) tabs: tabs::Tabs,
    pub(crate) pending_open: Option<PathBuf>,
    pub(crate) vault_catalog: Option<VaultCatalog>,
    pub(crate) catalog_updates: Option<std::sync::mpsc::Receiver<CatalogEvent>>,
    vault_load_generation: u64, // prevent bug when switching vault
    vault_db_ready_notified: bool,
    catalog_load_task: Task<()>,
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
    rename_sub: Option<Subscription>,
    pub(crate) drag_hover: Option<(PathBuf, Instant)>,
    pub(crate) expanded_tree_ids: Vec<SharedString>,
    pub(crate) focus_sidebar_requested: bool,
    pub(crate) focus_editor_requested: bool,
    sidebar_focus_handle: FocusHandle,
    pub(crate) last_sidebar_selection: Option<PathBuf>,
    pub(crate) pending_navigation: Option<tabs::NavigationAction>,
    pub(crate) pending_notifications: Vec<Notification>,
    pub(crate) registry: FileRegistry,
    pub(crate) startup: Option<Entity<StartupAnimation>>,
    startup_driver: Task<()>,
}

fn build_vault_entries() -> Vec<VaultEntry> {
    let mut items = Vec::new();
    let docs_vault = crate::app::docs::docs_vault_path();
    if docs_vault.is_dir() {
        items.push(VaultEntry::Vault {
            path: docs_vault.to_string_lossy().to_string().into(),
            name: SharedString::from(crate::app::docs::DOCS_VAULT_NAME),
        });
    }
    for path in app_settings::snapshot().recent_vaults {
        if path == docs_vault {
            continue;
        }
        let path_text: SharedString = path.to_string_lossy().to_string().into();
        let name: SharedString = display_name(&path).into();
        items.push(VaultEntry::Vault {
            path: path_text,
            name,
        });
    }
    items.push(VaultEntry::OpenNew(SharedString::from(VAULT_SELECT_MARKER)));
    items
}

fn is_current_vault_load(
    current_generation: u64,
    current_root: Option<&Path>,
    load_generation: u64,
    load_root: &Path,
) -> bool {
    current_generation == load_generation && current_root == Some(load_root)
}

impl DatalithView {
    #[must_use]
    pub(crate) fn new(
        first_startup: bool,
        initial_notifications: Vec<Notification>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let palette = Palette::new(window, cx);
        let palette_sub = Palette::input_subscription(&palette.input, window, cx);
        let sidebar_focus_handle = cx.focus_handle();
        let tree_state = cx.new(|cx| TreeState::new(cx));

        let vault_select_state =
            cx.new(|cx| SelectState::new(build_vault_entries(), None, window, cx));

        let vault_select_sub = cx.subscribe_in(
            &vault_select_state,
            window,
            |view: &mut Self, _state, event: &SelectEvent<Vec<VaultEntry>>, window, cx| match event
            {
                SelectEvent::Confirm(value) => {
                    if let Some(value) = value {
                        if value == VAULT_SELECT_MARKER {
                            window.dispatch_action(Box::new(crate::app::actions::OpenVault), cx);
                        } else {
                            let path = PathBuf::from(value.to_string());
                            view.set_root_path(path, cx);
                        }
                    }
                }
            },
        );

        let settings = SettingsView::new(cx);
        let font_size_slider_sub = cx.subscribe(
            &settings.font_size_slider_state,
            |view, _, event: &SliderEvent, cx| {
                let SliderEvent::Change(value) = event else {
                    return;
                };
                let val = f64::from(value.start());
                let new_size = px(BASE_FONT_SIZE * value.start());
                cx.global_mut::<settings::ThemeOptions>()
                    .font_size_multiplier = val;
                gpui_component::Theme::global_mut(cx).font_size = new_size;
                cx.refresh_windows();
                if let Err(error) = app_settings::set_font_scale(val) {
                    view.pending_notifications
                        .push(notifications::settings_save_failed("font scale", &error));
                }
            },
        );

        let startup = cx.new(|cx| {
            let kind = if first_startup {
                StartupType::First
            } else {
                StartupType::Standard
            };
            StartupAnimation::new(kind, cx)
        });

        let mut view = Self {
            tree_state,
            vault_select_state,
            root_path: None,
            root_name: "No folder open".into(),
            tabs: tabs::Tabs::new(),
            vault_catalog: None,
            catalog_updates: None,
            vault_load_generation: 0,
            vault_db_ready_notified: false,
            catalog_load_task: Task::ready(()),
            catalog_poll_task: Task::ready(()),
            pending_external_updates: Vec::new(),
            palette,
            _palette_sub: palette_sub,
            settings,
            _font_size_slider_sub: font_size_slider_sub,
            rename_sub: None,
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
            pending_notifications: initial_notifications,
            registry: registry::default_registry(),
            startup: Some(startup),
            startup_driver: Task::ready(()),
        };
        view.spawn_startup_driver(cx);
        view
    }

    pub(crate) fn set_root_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.vault_load_generation = self.vault_load_generation.wrapping_add(1);
        let generation = self.vault_load_generation;
        self.root_name = display_name(&path).into();
        self.root_path = Some(path.clone());
        if let Err(error) = app_settings::record_opened_vault(&path) {
            self.pending_notifications
                .push(notifications::settings_save_failed("opened vault", &error));
        }

        self.pending_vault_refresh = true;
        self.expanded_tree_ids.clear();
        self.pending_external_updates.clear();
        self.vault_catalog = None;
        self.catalog_updates = None;
        self.catalog_poll_task = Task::ready(());
        self.vault_db_ready_notified = false;

        let items = build_file_items_with_expanded(&path, &self.expanded_tree_ids);
        self.tree_state.update(cx, |state, cx| {
            state.set_items(items, cx);
        });
        if self.palette.open {
            let open_files = self.tabs.open_paths();
            self.palette.refresh(None, &open_files);
        }

        let file_types = self.registry.registered_file_types();
        let catalog_root = path.clone();
        let catalog_task =
            cx.background_spawn(async move { VaultCatalog::open(catalog_root, file_types) });
        self.catalog_load_task = cx.spawn(async move |this, cx| {
            let result = catalog_task.await;
            let _ = this.update(cx, |view, cx| {
                if !is_current_vault_load(
                    view.vault_load_generation,
                    view.root_path.as_deref(),
                    generation,
                    &path,
                ) {
                    return;
                }
                match result {
                    Ok(catalog) => {
                        view.catalog_updates = Some(catalog.events());
                        view.vault_catalog = Some(catalog);
                        view.start_catalog_polling(cx);
                        if view.palette.open {
                            let open_files = view.tabs.open_paths();
                            view.palette
                                .refresh(view.vault_catalog.as_ref(), &open_files);
                        }
                        view.pending_notifications
                            .push(notifications::catalog_loading());
                    }
                    Err(_) => {
                        view.pending_notifications
                            .push(notifications::vault_db_failed_to_load());
                    }
                }
                cx.notify();
            });
        });

        cx.notify();
    }

    fn start_catalog_polling(&mut self, cx: &Context<Self>) {
        self.catalog_poll_task = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(200))
                    .await;
                if this
                    .update(cx, |view, cx| {
                        let mut changed_paths = Vec::new();
                        let mut catalog_changed = false;
                        let mut structure_changed = false;
                        if let Some(ref updates) = view.catalog_updates {
                            while let Ok(update) = updates.try_recv() {
                                catalog_changed = true;
                                structure_changed |= update.structure_changed;
                                changed_paths.extend(update.paths.iter().cloned());
                            }
                        }

                        if catalog_changed {
                            let catalog_state =
                                view.vault_catalog.as_ref().map(VaultCatalog::state);
                            match catalog_state {
                                Some(CatalogState::Ready) => {
                                    if let (Some(catalog), Some(root)) =
                                        (view.vault_catalog.clone(), view.root_path.clone())
                                    {
                                        let handlers = view
                                            .tabs
                                            .iter()
                                            .filter(|(_, tab_path, _)| tab_path.starts_with(&root))
                                            .map(|(_, _, handler)| handler.clone())
                                            .collect::<Vec<_>>();
                                        for handler in handlers {
                                            let catalog = catalog.clone();
                                            handler.update(cx, |handler, cx| {
                                                handler.set_vault_catalog(catalog, cx);
                                            });
                                        }
                                    }
                                    if !view.vault_db_ready_notified {
                                        view.vault_db_ready_notified = true;
                                        view.pending_notifications
                                            .push(notifications::vault_db_ready());
                                    }
                                    view.refresh_tree(cx);
                                    structure_changed = false;
                                }
                                Some(CatalogState::Failed) => {
                                    view.pending_notifications
                                        .push(notifications::vault_db_failed_to_load());
                                }
                                _ => {}
                            }
                        }

                        if !changed_paths.is_empty() {
                            for removed in changed_paths.iter().filter(|path| !path.exists()) {
                                view.close_tabs_under(removed, cx);
                            }
                            view.pending_external_updates.extend(changed_paths);
                            if structure_changed {
                                view.refresh_tree(cx);
                            }
                            cx.notify();
                        }
                        if catalog_changed && view.palette.open {
                            let open_files = view.tabs.open_paths();
                            view.palette
                                .refresh(view.vault_catalog.as_ref(), &open_files);
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
    }

    pub(crate) fn refresh_vault_select(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.vault_select_state.update(cx, |state, cx| {
            state.set_items(build_vault_entries(), window, cx);
        });
        self.pending_vault_refresh = false;
    }

    pub(crate) fn create_quick_file(&mut self, extension: &str, cx: &mut Context<Self>) {
        let Some(root) = self.root_path.clone() else {
            return;
        };
        let base_name = format!("New Note.{extension}");
        match crate::vault::file_ops::create_with_name(&root, &base_name) {
            Ok(path) => {
                self.pending_open = Some(path);
            }
            Err(error) => {
                notifications::push_window_notification(
                    cx,
                    notifications::create_file_failed(&base_name, &error),
                );
            }
        }
        cx.notify();
    }

    #[must_use]
    pub(crate) fn resolve_target(&self, cx: &Context<Self>) -> Option<PathBuf> {
        self.tree_state
            .read(cx)
            .selected_entry()
            .map(|e| PathBuf::from(e.item().id.to_string()))
            .or_else(|| self.tabs.active_path().map(Path::to_path_buf))
            .or_else(|| self.last_sidebar_selection.clone())
    }

    pub(crate) fn refresh_tree(&self, cx: &mut Context<Self>) {
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

    pub(crate) fn refresh_tree_with_rename(
        &mut self,
        old_path: &Path,
        new_path: &Path,
        cx: &mut Context<Self>,
    ) {
        fn remap(path: &Path, old_path: &Path, new_path: &Path) -> PathBuf {
            path.strip_prefix(old_path)
                .map_or_else(|_| path.to_path_buf(), |suffix| new_path.join(suffix))
        }

        fn remap_items(items: &mut [TreeItem], old_path: &Path, new_path: &Path) {
            for item in items {
                let path = PathBuf::from(item.id.to_string());
                if path == old_path || path.starts_with(old_path) {
                    let renamed = remap(&path, old_path, new_path);
                    item.id = renamed.to_string_lossy().to_string().into();
                    if path == old_path {
                        item.label = display_name(new_path).to_string().into();
                    }
                }
                remap_items(&mut item.children, old_path, new_path);
            }
        }

        let Some(ref root) = self.root_path else {
            return;
        };
        let selected = self.tree_state.read(cx).selected_entry().map(|entry| {
            remap(
                &PathBuf::from(entry.item().id.to_string()),
                old_path,
                new_path,
            )
        });
        let mut items = build_file_items_with_expanded(root, &self.expanded_tree_ids);
        remap_items(&mut items, old_path, new_path);
        for expanded in &mut self.expanded_tree_ids {
            let renamed = remap(&PathBuf::from(expanded.to_string()), old_path, new_path);
            *expanded = renamed.to_string_lossy().to_string().into();
        }
        self.tree_state.update(cx, |state, cx| {
            state.set_items(items, cx);
            if let Some(selected) = selected {
                let selected = TreeItem::new(
                    selected.to_string_lossy().to_string(),
                    display_name(&selected).to_string(),
                );
                state.set_selected_item(Some(&selected), cx);
            }
        });
        cx.notify();
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
        // Counting tree nodes cannot plausibly overflow for real vault sizes.
        #[allow(clippy::arithmetic_side_effects)]
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

        self.root_path.as_ref().map_or(0, |root| {
            count_items(&build_file_items_with_expanded(
                root,
                &self.expanded_tree_ids,
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::is_current_vault_load;
    use std::path::Path;

    #[test]
    fn only_the_current_vault_load_can_publish_results() {
        let current = Path::new("/vault/current");

        assert!(is_current_vault_load(2, Some(current), 2, current));
        assert!(!is_current_vault_load(2, Some(current), 1, current));
        assert!(!is_current_vault_load(
            2,
            Some(current),
            2,
            Path::new("/vault/previous")
        ));
    }
}
