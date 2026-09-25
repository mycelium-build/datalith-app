pub mod editors;
pub mod icons;
pub mod licenses;
pub mod monolith;
pub mod notifications;
pub mod palette;
pub mod render;
mod session;
pub mod settings;
pub mod sidebar;
pub mod startup;
pub mod tabs;
pub mod themes;
pub mod title_bar;
pub mod viewers;
pub mod window;

pub const BASE_FONT_SIZE: f32 = 16.0;
const LINE_HEIGHT: f32 = 1.6;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gpui_kit::component::{
    input::InputState,
    notification::Notification,
    slider::SliderEvent,
    tree::{TreeEvent, TreeState},
};
use gpui_kit::{
    AppContext, Context, Entity, FocusHandle, SharedString, Subscription, Task, Window, px,
};

use crate::app::settings as app_settings;
use crate::document::registry::{self, FileRegistry};
use crate::ui::startup::{StartupAnimation, StartupType};
use crate::vault::{CatalogEvent, CatalogState, VaultCatalog};
use palette::Palette;
use settings::SettingsView;

pub enum PendingOpen {
    Open(PathBuf),
    Created(PathBuf),
}

// The view tracks several independent one-shot UI flags (focus requests, refresh notifications) that are read and cleared during rendering;
// grouping them would obscure the render loop's intent.
#[allow(clippy::struct_excessive_bools)]
pub struct DatalithView {
    update_control: Option<Entity<title_bar::UpdateControl>>,
    pub(crate) tree_state: Entity<TreeState>,
    _tree_state_sub: Subscription,
    pub(crate) root_path: Option<PathBuf>,
    root_name: SharedString,
    pub(crate) tabs: tabs::Tabs,
    pub(crate) pending_open: Option<PendingOpen>,
    workspace_machine_id: Option<String>,
    workspace_save_blocked: bool,
    vault_transition_generation: u64,
    pending_vault_path: Option<PathBuf>,
    pending_vault_open: Option<PendingOpen>,
    vault_transition_task: Task<()>,
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
    _appearance_sub: Subscription,
    pub(crate) licenses: licenses::LicensesView,
    pub(crate) context_menu_target: Option<PathBuf>,
    pub(crate) context_menu_from_row: bool,
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
    app_menu_bar: Entity<title_bar::ApplicationMenu>,
    pub(crate) startup: Option<Entity<StartupAnimation>>,
    startup_driver: Task<()>,
}

impl DatalithView {
    #[must_use]
    pub(crate) fn new(
        first_startup: bool,
        initial_notifications: Vec<Notification>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let app_menu_bar = cx.new(|cx| title_bar::ApplicationMenu::new(window, cx));
        let palette = Palette::new(window, cx);
        let palette_sub = Palette::input_subscription(&palette.input, window, cx);
        let sidebar_focus_handle = cx.focus_handle();
        let tree_state = cx.new(|cx| TreeState::new(cx));
        let tree_state_sub = Self::subscribe_tree_events(&tree_state, window, cx);

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
                gpui_kit::component::Theme::global_mut(cx).font_size = new_size;
                cx.refresh_windows();
                if let Err(error) = app_settings::set_font_scale(val) {
                    view.pending_notifications
                        .push(notifications::settings_save_failed("font scale", &error));
                }
            },
        );

        let appearance_sub = Self::observe_system_appearance(window, cx);

        let startup = cx.new(|cx| {
            let kind = if first_startup {
                StartupType::First
            } else {
                StartupType::Standard
            };
            StartupAnimation::new(kind, cx)
        });

        let update_control = crate::app::update::Updater::get(cx)
            .map(|updater| cx.new(|cx| title_bar::UpdateControl::new(updater, cx)));
        let workspace_machine_id = crate::app::workspace::machine_id().ok();
        let mut view = Self {
            update_control,
            tree_state,
            root_path: None,
            root_name: "No vault opened".into(),
            tabs: tabs::Tabs::new(),
            pending_open: None,
            workspace_machine_id,
            workspace_save_blocked: false,
            vault_transition_generation: 0,
            pending_vault_path: None,
            pending_vault_open: None,
            vault_transition_task: Task::ready(()),
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
            _appearance_sub: appearance_sub,
            licenses: licenses::LicensesView::new(cx),
            rename_sub: None,
            _tree_state_sub: tree_state_sub,
            context_menu_target: None,
            context_menu_from_row: false,
            rename_target: None,
            rename_state: None,
            drag_hover: None,
            expanded_tree_ids: Vec::new(),
            focus_sidebar_requested: false,
            focus_editor_requested: false,
            sidebar_focus_handle,
            last_sidebar_selection: None,
            pending_navigation: None,
            pending_notifications: initial_notifications,
            registry: registry::default_registry(),
            app_menu_bar,
            startup: Some(startup),
            startup_driver: Task::ready(()),
        };
        view.spawn_startup_driver(cx);
        view
    }

    fn subscribe_tree_events(
        tree_state: &Entity<TreeState>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        // gpui-base mutates expansion behind our back
        // (e.g. `expand_ancestors` while restoring a selection);
        // keep `expanded_tree_ids` in sync.
        cx.subscribe_in(
            tree_state,
            window,
            |view: &mut Self, _state, event: &TreeEvent, _window, _cx| match event {
                TreeEvent::Expanded(id) => view.mark_tree_item_expanded(id, true),
                TreeEvent::Collapsed(id) => view.mark_tree_item_expanded(id, false),
            },
        )
    }

    fn observe_system_appearance(window: &mut Window, cx: &Context<Self>) -> Subscription {
        cx.observe_window_appearance(window, |_, window, cx| {
            let preference = app_settings::snapshot().theme_preference;
            if preference == app_settings::ThemePreference::System {
                let effective = preference.resolve(window.appearance()).into();
                gpui_kit::component::Theme::change(effective, Some(window), cx);
                gpui_kit::component::Theme::global_mut(cx).mode = effective;
            }
        })
    }

    fn use_vault_catalog(&mut self, catalog: VaultCatalog, cx: &Context<Self>) {
        self.catalog_updates = Some(catalog.events());
        self.vault_catalog = Some(catalog);
        self.start_catalog_polling(cx);
        if self.palette.open {
            self.palette
                .refresh(self.vault_catalog.as_ref(), &self.tabs.open_paths());
        }
        self.pending_notifications
            .push(notifications::catalog_loading());
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
                        let mut catalog_changed = !view.vault_db_ready_notified
                            && view
                                .vault_catalog
                                .as_ref()
                                .is_some_and(|catalog| catalog.state() == CatalogState::Ready);
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
                                        // Recheck restored documents once when the catalog is ready.
                                        if !view.vault_db_ready_notified {
                                            changed_paths.extend(
                                                view.tabs
                                                    .iter()
                                                    .filter(|(_, path, _)| path.starts_with(&root))
                                                    .map(|(_, path, _)| path.to_path_buf()),
                                            );
                                        }
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
                            for removed in changed_paths
                                .iter()
                                .filter(|path| !crate::vault::source::exists(path))
                            {
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

    pub(crate) fn create_quick_file(&mut self, extension: &str, cx: &mut Context<Self>) {
        let Some(root) = self.root_path.clone() else {
            return;
        };
        let base_name = format!("New Note.{extension}");
        match crate::vault::file_ops::create_with_name(&root, &base_name) {
            Ok(path) => {
                if extension == "base"
                    && let Err(error) = crate::vault::file_ops::update(
                        &path,
                        "views:\n  - type: table\n    name: All files\n",
                    )
                {
                    notifications::push_window_notification(
                        cx,
                        notifications::create_file_failed(&base_name, &error),
                    );
                    return;
                }
                self.pending_open = Some(PendingOpen::Created(path));
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
}
