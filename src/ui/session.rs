use gpui_kit::component::tree::TreeItem;
use gpui_kit::{App, AppContext, Context, Task, Window};
use std::path::{Path, PathBuf};

use crate::app::{
    settings,
    workspace::{TabId, Workspace, WorkspaceTab},
};
use crate::document::handler::ViewMode;
use crate::ui::{DatalithView, PendingOpen};
use crate::vault::VaultCatalog;

fn same_vault_path(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn initial_workspace(root: &Path, first_startup: bool) -> Workspace {
    let welcome = root.join(crate::app::docs::WELCOME_NOTE);
    let tabs = if first_startup && welcome.is_file() {
        vec![WorkspaceTab::new(
            TabId::new(),
            Some(welcome),
            ViewMode::View,
        )]
    } else {
        Vec::new()
    };
    let active_tab_id = tabs.first().map(|tab| tab.id().clone());
    Workspace {
        tabs,
        active_tab_id,
        ..Workspace::default()
    }
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
    pub(super) fn restore_initial_workspace(
        &mut self,
        path: Option<PathBuf>,
        first_startup: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = path.filter(|path| path.is_dir()) else {
            return;
        };

        let (workspace, save_blocked) = match self.load_workspace(&path) {
            Ok(Some(workspace)) => (workspace, false),
            Ok(None) => (initial_workspace(&path, first_startup), false),
            Err(error) => {
                self.pending_notifications
                    .push(super::notifications::workspace_load_failed(&error));
                (initial_workspace(&path, first_startup), true)
            }
        };
        self.activate_vault(path, workspace, save_blocked, None, window, cx);
    }

    pub(crate) fn set_root_path(
        &mut self,
        path: PathBuf,
        pending_open: Option<PendingOpen>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self
            .root_path
            .as_deref()
            .is_some_and(|active| same_vault_path(active, &path))
        {
            self.cancel_pending_vault_transition();
            if let Some(pending_open) = pending_open {
                self.pending_open = Some(pending_open);
                cx.notify();
            }
            return true;
        }

        if path.is_dir()
            && self
                .pending_vault_path
                .as_deref()
                .is_some_and(|pending| same_vault_path(pending, &path))
        {
            if pending_open.is_some() {
                self.pending_vault_open = pending_open;
            }
            return true;
        }

        // Every distinct request supersedes an in-flight transition, including a
        // request that later fails validation, saving, or workspace loading.
        self.cancel_pending_vault_transition();

        if !path.is_dir() {
            self.pending_notifications
                .push(super::notifications::workspace_load_failed(
                    &anyhow::anyhow!("the selected folder is unavailable"),
                ));
            cx.notify();
            return false;
        }

        if self.root_path.is_some()
            && let Err(error) = self.save_workspace(cx)
        {
            self.pending_notifications
                .push(super::notifications::workspace_save_failed(&error));
            cx.notify();
            return false;
        }

        let workspace = match self.load_workspace(&path) {
            Ok(workspace) => workspace.unwrap_or_default(),
            Err(error) => {
                self.pending_notifications
                    .push(super::notifications::workspace_load_failed(&error));
                cx.notify();
                return false;
            }
        };

        // Open the target catalog before releasing the current workspace. A catalog
        // initialization error therefore leaves the active vault and its tabs intact.
        self.vault_transition_generation = self.vault_transition_generation.wrapping_add(1);
        let generation = self.vault_transition_generation;
        let source_root = self.root_path.clone();
        self.pending_vault_path = Some(path.clone());
        self.pending_vault_open = pending_open;
        let catalog_root = path.clone();
        let file_types = self.registry.registered_file_types();
        let catalog_load = cx.background_spawn(async move {
            anyhow::ensure!(catalog_root.is_dir(), "Vault folder is unavailable");
            VaultCatalog::open(catalog_root, file_types)
        });
        let window_handle = window.window_handle();
        self.vault_transition_task = cx.spawn(async move |this, cx| {
            let catalog_result = catalog_load.await;
            let _ = window_handle.update(cx, |_, window, app| {
                let _ = this.update(app, |view, cx| {
                    if view.vault_transition_generation != generation
                        || view.root_path != source_root
                    {
                        return;
                    }

                    let Ok(catalog) = catalog_result else {
                        view.cancel_pending_vault_transition();
                        view.pending_notifications
                            .push(super::notifications::vault_db_failed_to_load());
                        cx.notify();
                        return;
                    };

                    // The user can continue working in A while B's catalog opens.
                    // Capture those changes before committing the transition.
                    if view.root_path.is_some()
                        && let Err(error) = view.save_workspace(cx)
                    {
                        view.cancel_pending_vault_transition();
                        view.pending_notifications
                            .push(super::notifications::workspace_save_failed(&error));
                        cx.notify();
                        return;
                    }

                    let pending_open = view.pending_vault_open.take();
                    view.pending_vault_path = None;
                    view.activate_vault(path, workspace, false, Some(catalog), window, cx);
                    view.pending_open = pending_open;
                    cx.notify();
                });
            });
        });
        true
    }

    fn cancel_pending_vault_transition(&mut self) {
        self.vault_transition_generation = self.vault_transition_generation.wrapping_add(1);
        self.vault_transition_task = Task::ready(());
        self.pending_vault_path = None;
        self.pending_vault_open = None;
    }

    fn load_workspace(&self, path: &Path) -> anyhow::Result<Option<Workspace>> {
        let machine_id = self
            .workspace_machine_id
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("the machine workspace identity is unavailable"))?;
        Workspace::load(path, machine_id)
    }

    pub(super) fn save_workspace(&self, cx: &App) -> anyhow::Result<()> {
        let Some(root) = &self.root_path else {
            return Ok(());
        };
        if self.workspace_save_blocked {
            anyhow::bail!("the saved workspace could not be read and was left untouched");
        }
        let machine_id = self
            .workspace_machine_id
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("the machine workspace identity is unavailable"))?;
        let (tabs, active_tab_id) = self.tabs.snapshot(cx);
        let workspace = Workspace {
            tabs,
            active_tab_id,
            expanded_folders: self
                .expanded_tree_ids
                .iter()
                .map(|id| PathBuf::from(id.as_str()))
                .collect(),
            sidebar_selection: self
                .tree_state
                .read(cx)
                .selected_entry()
                .map(|entry| PathBuf::from(entry.item().id.as_str())),
        };
        workspace.save(root, machine_id)
    }

    fn activate_vault(
        &mut self,
        path: PathBuf,
        workspace: Workspace,
        save_blocked: bool,
        catalog: Option<VaultCatalog>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_pending_vault_transition();
        self.vault_load_generation = self.vault_load_generation.wrapping_add(1);
        let generation = self.vault_load_generation;

        // Clear A's catalog before creating B's handlers so Base tabs cannot keep
        // the previous vault's catalog through their initial dependencies.
        self.vault_catalog = None;
        self.catalog_load_task = Task::ready(());
        self.catalog_updates = None;
        self.catalog_poll_task = Task::ready(());
        self.vault_db_ready_notified = false;

        self.root_path = Some(path.clone());
        self.root_name = crate::vault::path::display_name(&path).into();
        self.workspace_save_blocked = save_blocked;
        if let Err(error) = settings::record_opened_vault(&path) {
            self.pending_notifications
                .push(super::notifications::settings_save_failed(
                    "opened vault",
                    &error,
                ));
        }

        self.pending_open = None;
        self.pending_vault_refresh = true;
        self.pending_external_updates.clear();
        self.context_menu_target = None;
        self.rename_target = None;
        self.rename_state = None;
        self.rename_sub = None;
        self.tabs.clear();
        self.expanded_tree_ids = workspace
            .expanded_folders
            .into_iter()
            .filter(|folder| folder.starts_with(&path) && folder.is_dir())
            .map(|folder| folder.to_string_lossy().into_owned().into())
            .collect();
        self.restore_workspace_tabs(workspace.tabs, workspace.active_tab_id.as_ref(), window, cx);
        self.last_sidebar_selection = None;
        self.refresh_tree(cx);
        if let Some(selection) = workspace
            .sidebar_selection
            .filter(|selection| selection.starts_with(&path) && selection.exists())
        {
            let item = TreeItem::new(selection.to_string_lossy().into_owned(), "");
            self.tree_state
                .update(cx, |state, cx| state.set_selected_item(Some(&item), cx));
            self.last_sidebar_selection = Some(selection);
        }
        if self.tabs.active_index().is_some() {
            self.focus_active_tab(window, cx);
        }
        if self.palette.open {
            self.palette.refresh(None, &self.tabs.open_paths());
        }

        if let Some(catalog) = catalog {
            self.use_vault_catalog(catalog, cx);
        } else {
            let file_types = self.registry.registered_file_types();
            let catalog_root = path.clone();
            let catalog_load = cx.background_spawn(async move {
                anyhow::ensure!(catalog_root.is_dir(), "Vault folder is unavailable");
                VaultCatalog::open(catalog_root, file_types)
            });
            self.catalog_load_task = cx.spawn(async move |this, cx| {
                let result = catalog_load.await;
                if let Err(error) = this.update(cx, |view, cx| {
                    if !is_current_vault_load(
                        view.vault_load_generation,
                        view.root_path.as_deref(),
                        generation,
                        &path,
                    ) {
                        return;
                    }
                    match result {
                        Ok(catalog) => view.use_vault_catalog(catalog, cx),
                        Err(_) => view
                            .pending_notifications
                            .push(super::notifications::vault_db_failed_to_load()),
                    }
                    cx.notify();
                }) {
                    eprintln!("Failed to publish Vault load result to the UI: {error}");
                }
            });
        }
        cx.notify();
    }
}
