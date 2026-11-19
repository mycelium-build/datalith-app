mod cards;
mod cells;
mod graph;
mod kanban;
mod list;
mod preview;
mod snapshot;
mod table;
mod touch;
mod touch_choice;

use cells::centered_message;
use cells::{format_scalar_text, render_property_cell};
use snapshot::file_name;
use snapshot::load_snapshot;
use snapshot::{BaseItem, BaseRow, BaseSnapshot, BaseStatus};

use gpui_kit::component::input::{EditorState, InputEvent, InputState};
use gpui_kit::component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};
use gpui_kit::{
    AnyElement, App, AppContext, Context, ElementId, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Render, Styled, Task, Window, div,
    prelude::FluentBuilder,
};

use gpui_kit::base::actions::{Cancel, SelectDown, SelectLeft, SelectRight, SelectUp};

use crate::actions::{NextView, PreviousView, ResetZoom, ZoomIn, ZoomOut};
use datalith_core::base::{BaseDefinition, ViewType};

/// How much one Zoom in or Zoom out scales the graph.
const ZOOM_STEP: f32 = 1.25;
/// How far one arrow key pans the graph, in pixels.
const GRAPH_PAN_STEP: f32 = 60.0;
use crate::handler::FileHandler;
use datalith_vault::VaultCatalog;
use datalith_vault::file_ops;

pub struct BaseViewer {
    state: Entity<BaseViewState>,
}

impl BaseViewer {
    pub(crate) fn dismiss_control(&self, cx: &mut App) -> bool {
        self.state.update(cx, |s, cx| {
            let dismissed = if s.choosing_view {
                s.choosing_view = false;
                true
            } else if let Some(rows) = s
                .selected_view
                .as_ref()
                .and_then(|name| s.touch_rows.get_mut(name))
                .filter(|rows| rows.choosing_property)
            {
                rows.choosing_property = false;
                true
            } else if let Some(graph) = s.graph.clone()
                && graph.update(cx, graph::GraphState::close_note_choice)
            {
                true
            } else if s.fullscreen_image.take().is_some() {
                true
            } else if let Some(graph) = s.graph.clone()
                && graph.update(cx, graph::GraphState::clear_selection)
            {
                true
            } else {
                s.selected_view
                    .as_ref()
                    .and_then(|name| s.touch_rows.get_mut(name))
                    .and_then(|rows| rows.detail.take())
                    .is_some()
            };
            if dismissed {
                cx.notify();
            }
            dismissed
        })
    }

    pub(crate) fn new(
        input: Entity<EditorState>,
        catalog: Option<VaultCatalog>,
        cx: &mut Context<FileHandler>,
    ) -> Self {
        let handler = cx.entity().downgrade();
        let state = cx.new(|cx| BaseViewState::new(input, catalog, handler, cx));
        state.update(cx, BaseViewState::rebuild);
        Self { state }
    }

    pub(super) fn refresh(&self, cx: &mut App) {
        self.state.update(cx, BaseViewState::rebuild);
    }

    pub(super) fn set_vault_catalog(&self, catalog: VaultCatalog, cx: &mut Context<FileHandler>) {
        self.state.update(cx, |state, cx| {
            state.catalog = Some(catalog);
            state.rebuild(cx);
        });
    }

    pub(super) fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.state.read(cx).focus_handle.clone()
    }

    pub(super) fn render(&self, _handler: Entity<FileHandler>, _cx: &mut App) -> AnyElement {
        self.state.clone().into_any_element()
    }
}

pub struct BaseViewState {
    /// Theme previews are read-only and take their palette from the scoped global theme.
    preview: bool,
    input: Entity<EditorState>,
    /// The parsed definition the switcher renders from and queries reuse;
    /// replaced whenever the source parses successfully again.
    definition: Option<BaseDefinition>,
    pub(super) catalog: Option<VaultCatalog>,
    handler: gpui_kit::WeakEntity<FileHandler>,
    status: BaseStatus,
    pub(super) focus_handle: FocusHandle,
    list: Option<list::ListState>,
    table: Option<table::TableState>,
    cards: Option<cards::CardsState>,
    kanban: Option<kanban::KanbanState>,
    graph: Option<Entity<graph::GraphState>>,
    /// Fullscreen card image, shared by views.
    fullscreen_image: Option<cards::CardImage>,
    /// The definition source the last successful build came from;
    /// graph camera/simulation reset only when this changes.
    last_source: String,
    selected_view: Option<String>,
    choosing_view: bool,
    touch_rows: std::collections::HashMap<String, touch::Rows>,
    view_states: std::collections::HashMap<String, touch::SavedView>,
    active_state: Option<String>,
    generation: u64,
    /// Monotonic identity handed to each stored snapshot,
    /// so viewers can cache derived data per snapshot instead of per render.
    next_snapshot_id: u64,
    build_task: Task<()>,
}

impl BaseViewState {
    fn new(
        input: Entity<EditorState>,
        catalog: Option<VaultCatalog>,
        handler: gpui_kit::WeakEntity<FileHandler>,
        cx: &Context<Self>,
    ) -> Self {
        Self {
            preview: false,
            input,
            definition: None,
            catalog,
            handler,
            status: BaseStatus::Loading,
            focus_handle: cx.focus_handle(),
            list: None,
            table: None,
            cards: None,
            kanban: None,
            graph: None,
            fullscreen_image: None,
            last_source: String::new(),
            selected_view: None,
            choosing_view: false,
            touch_rows: std::collections::HashMap::new(),
            view_states: std::collections::HashMap::new(),
            active_state: None,
            generation: 0,
            next_snapshot_id: 0,
            build_task: Task::ready(()),
        }
    }

    fn rebuild(&mut self, cx: &mut Context<Self>) {
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let source = self.input.read(cx).value().to_string();
        match BaseDefinition::parse(&source) {
            Ok(definition) => {
                self.definition = Some(definition.clone());
                self.spawn_query(definition, generation, cx);
            }
            Err(error) => {
                self.definition = None;
                self.status = BaseStatus::Error(error.to_string());
                cx.notify();
            }
        }
    }

    /// Queries the catalog for the held definition's selected view;
    /// switching views calls this directly, skipping the source re-parse.
    fn spawn_query(&mut self, definition: BaseDefinition, generation: u64, cx: &mut Context<Self>) {
        if let Some(list) = &mut self.list {
            list.item_sizes.clear();
        }
        if let Some(table) = &mut self.table {
            table.item_sizes.clear();
        }
        let selected_name = self.selected_view.clone();
        let Some(catalog) = self.catalog.clone() else {
            self.status = BaseStatus::Error("No Vault Catalog is available".into());
            cx.notify();
            return;
        };
        let source = self.input.read(cx).value().to_string();
        let source_changed = self.last_source != source;
        self.status = BaseStatus::Loading;
        self.build_task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    load_snapshot(definition, selected_name.as_deref(), catalog).await
                })
                .await;
            let _ = this.update(cx, |state, cx| {
                if state.generation != generation {
                    return;
                }
                match result {
                    Ok(mut snapshot) => {
                        state.next_snapshot_id = state.next_snapshot_id.wrapping_add(1);
                        snapshot.id = state.next_snapshot_id;
                        state.last_source.clone_from(&source);
                        state.restore_mobile_view(&snapshot);
                        state.selected_view = snapshot
                            .definition
                            .views
                            .get(snapshot.view_index)
                            .map(|view| view.name.clone());
                        if cfg!(any(target_os = "ios", target_os = "android"))
                            && let Some(name) = &state.selected_view
                        {
                            state.touch_rows.entry(name.clone()).or_default();
                        }
                        match snapshot.definition.views.get(snapshot.view_index) {
                            Some(view) if view.view_type == ViewType::List => {
                                state
                                    .list
                                    .get_or_insert_with(list::ListState::new)
                                    .item_sizes = list::row_sizes(&snapshot);
                            }
                            Some(view) if view.view_type == ViewType::Table => {
                                state
                                    .table
                                    .get_or_insert_with(table::TableState::new)
                                    .item_sizes = table::row_sizes(&snapshot);
                            }
                            Some(view) if view.view_type == ViewType::Cards => {
                                state.cards.get_or_insert_with(cards::CardsState::new);
                            }
                            Some(view) if view.view_type == ViewType::Kanban => {
                                let columns = snapshot
                                    .items
                                    .iter()
                                    .filter(|item| matches!(item, BaseItem::Header { .. }))
                                    .count();
                                state
                                    .kanban
                                    .get_or_insert_with(kanban::KanbanState::new)
                                    .ensure_columns(columns);
                            }
                            Some(view) if view.view_type == ViewType::Graph => {
                                state.update_graph(&snapshot, view, source_changed, cx);
                            }
                            _ => {}
                        }
                        state.status = if snapshot.rows.is_empty() {
                            BaseStatus::Empty(snapshot)
                        } else {
                            BaseStatus::Ready(snapshot)
                        };
                    }
                    Err(error) => state.status = BaseStatus::Error(error.to_string()),
                }
                cx.notify();
            });
        });
        cx.notify();
    }

    pub(super) fn begin_cell_edit(
        &mut self,
        is_list: bool,
        row: usize,
        column: usize,
        initial: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.preview {
            return;
        }
        self.clear_edits();
        let input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_value(initial.to_string(), window, cx);
            state
        });
        let subscription = cx.subscribe_in(
            &input,
            window,
            move |this, _input, event, _window, cx| match event {
                InputEvent::PressEnter { .. } | InputEvent::Blur => {
                    this.commit_cell_edit(cx);
                }
                _ => {}
            },
        );
        input.focus_handle(cx).focus(window, cx);
        let edit = cells::CellEdit {
            row,
            column,
            input,
            initial: initial.to_owned(),
            subscription: Some(subscription),
            error: None,
        };
        if is_list {
            if let Some(list) = &mut self.list {
                list.edit = Some(edit);
            }
        } else if let Some(table) = &mut self.table {
            table.edit = Some(edit);
        }
        cx.notify();
    }

    fn clear_edits(&mut self) {
        if let Some(list) = &mut self.list {
            list.edit = None;
        }
        if let Some(table) = &mut self.table {
            table.edit = None;
        }
    }

    /// Removes the active edit from its owning view, returning the view kind plus the edit
    /// (subscription included, so it stays alive for rollback).
    fn take_edit(&mut self) -> Option<(bool, cells::CellEdit)> {
        if let Some(list) = &mut self.list
            && let Some(edit) = list.edit.take()
        {
            return Some((true, edit));
        }
        if let Some(table) = &mut self.table
            && let Some(edit) = table.edit.take()
        {
            return Some((false, edit));
        }
        None
    }

    fn restore_edit(&mut self, is_list: bool, edit: cells::CellEdit) {
        if is_list {
            if let Some(list) = &mut self.list {
                list.edit = Some(edit);
            }
        } else if let Some(table) = &mut self.table {
            table.edit = Some(edit);
        }
    }

    /// Commits the active edit: fs write -> catalog reconcile -> explicit re-query,
    /// so the view reflects the change immediately.
    /// On failure the input stays open with the error message.
    fn commit_cell_edit(&mut self, cx: &mut Context<Self>) {
        let Some((is_list, mut edit)) = self.take_edit() else {
            return;
        };
        let new_value = edit.input.read(cx).value().to_string();
        if new_value == edit.initial {
            cx.notify();
            return;
        }
        let snapshot = match &self.status {
            BaseStatus::Ready(snapshot) | BaseStatus::Empty(snapshot) => snapshot.clone(),
            _ => {
                edit.error = Some("No data to edit".into());
                self.restore_edit(is_list, edit);
                cx.notify();
                return;
            }
        };
        let Some(row) = snapshot.rows.get(edit.row).cloned() else {
            cx.notify();
            return;
        };
        let Some(view) = snapshot.definition.views.get(snapshot.view_index).cloned() else {
            cx.notify();
            return;
        };
        let Some(property) = view.order.get(edit.column).cloned() else {
            cx.notify();
            return;
        };
        let Some(catalog) = self.catalog.clone() else {
            edit.error = Some("No Vault Catalog is available".into());
            self.restore_edit(is_list, edit);
            cx.notify();
            return;
        };
        if !catalog.has_yaml_frontmatter(&row.path) {
            edit.error = Some("This file type has no YAML frontmatter".into());
            self.restore_edit(is_list, edit);
            cx.notify();
            return;
        }
        let result = edited_value(
            snapshot.projection_value(&row, &property.source),
            &new_value,
        )
        .and_then(|value| {
            file_ops::update_frontmatter(
                &catalog.root().join(&row.path),
                &property.source,
                value.as_ref(),
            )
        });
        match result {
            Ok(()) => {
                // fs write is not enough: DB reconcile is async (notify -> reconciler thread).
                // Force a route through it so the re-query below does not read stale frontmatter.
                catalog.refresh_paths(vec![catalog.root().join(&row.path)]);
                self.rebuild(cx);
            }
            Err(error) => {
                edit.error = Some(error.to_string());
                self.restore_edit(is_list, edit);
                cx.notify();
            }
        }
    }

    pub(super) fn cancel_cell_edit(&mut self, cx: &mut Context<Self>) {
        self.clear_edits();
        cx.notify();
    }

    /// Selects the next view, or the previous one, wrapping around the ends.
    fn select_adjacent_view(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(views) = self.definition.as_ref().map(|definition| &definition.views) else {
            return;
        };
        let current = self
            .selected_view
            .as_ref()
            .and_then(|name| views.iter().position(|view| &view.name == name))
            .unwrap_or(0);
        let next = if forward {
            current
                .checked_add(1)
                .filter(|next| *next < views.len())
                .unwrap_or(0)
        } else {
            current
                .checked_sub(1)
                .unwrap_or_else(|| views.len().saturating_sub(1))
        };
        if let Some(name) = views.get(next).map(|view| view.name.clone())
            && next != current
        {
            self.select_view(name, cx);
        }
    }

    /// The vertical scroll of the current view, when it scrolls as a whole.
    fn shown_scroll_handle(&self) -> Option<gpui_kit::ScrollHandle> {
        let BaseStatus::Ready(snapshot) = &self.status else {
            return None;
        };
        let view = snapshot.definition.views.get(snapshot.view_index)?;
        let handle = match view.view_type {
            ViewType::List => &self.list.as_ref()?.scroll_handle,
            ViewType::Table => &self.table.as_ref()?.scroll_handle,
            ViewType::Cards => &self.cards.as_ref()?.scroll_handle,
            ViewType::Kanban | ViewType::Graph => return None,
        };
        Some((**handle).clone())
    }

    /// The graph, when the current view shows it.
    fn shown_graph(&self) -> Option<Entity<graph::GraphState>> {
        let BaseStatus::Ready(snapshot) = &self.status else {
            return None;
        };
        let view = snapshot.definition.views.get(snapshot.view_index)?;
        matches!(view.view_type, ViewType::Graph)
            .then(|| self.graph.clone())
            .flatten()
    }

    /// Pans the shown graph with the arrow keys.
    fn with_graph_keys(element: gpui_kit::Div, cx: &Context<Self>) -> gpui_kit::Div {
        let pan = |x: f32, y: f32| {
            move |this: &mut Self, cx: &mut Context<Self>| {
                this.update_shown_graph(cx, |graph, cx| graph.pan_by(x, y, cx));
            }
        };
        let (up, down, left, right) = (
            pan(0.0, GRAPH_PAN_STEP),
            pan(0.0, -GRAPH_PAN_STEP),
            pan(GRAPH_PAN_STEP, 0.0),
            pan(-GRAPH_PAN_STEP, 0.0),
        );
        element
            .key_context(crate::contexts::BASE_GRAPH_CONTEXT)
            .on_action(cx.listener(move |this, _: &SelectUp, _, cx| up(this, cx)))
            .on_action(cx.listener(move |this, _: &SelectDown, _, cx| down(this, cx)))
            .on_action(cx.listener(move |this, _: &SelectLeft, _, cx| left(this, cx)))
            .on_action(cx.listener(move |this, _: &SelectRight, _, cx| right(this, cx)))
    }

    fn update_shown_graph(
        &self,
        cx: &mut Context<Self>,
        update: impl FnOnce(&mut graph::GraphState, &mut Context<graph::GraphState>),
    ) {
        match self.shown_graph() {
            Some(graph) => graph.update(cx, update),
            None => cx.propagate(),
        }
    }

    fn select_view(&mut self, name: String, cx: &mut Context<Self>) {
        self.clear_edits();
        self.selected_view = Some(name);
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        match self.definition.clone() {
            Some(definition) => self.spawn_query(definition, generation, cx),
            None => self.rebuild(cx),
        }
    }

    /// Feeds the embedded graph viewer with rows from a graph-view snapshot.
    fn update_graph(
        &mut self,
        snapshot: &BaseSnapshot,
        view: &datalith_core::base::BaseView,
        reset_view: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(config) = view.as_graph() else {
            return;
        };
        let root = self
            .catalog
            .as_ref()
            .map_or_else(std::path::PathBuf::new, VaultCatalog::root);
        let rows = snapshot
            .rows
            .iter()
            .map(|row| (row.path.clone(), row.links.clone(), row.class_hits.clone()));
        let summary_lines: Vec<String> = snapshot
            .summaries
            .iter()
            .map(snapshot::summary_entry_text)
            .collect();
        let built = graph::build_graph_snapshot(config, &root, rows, summary_lines);
        let has_nodes = !snapshot.rows.is_empty();
        let entity = self.graph.get_or_insert_with(|| {
            let handler = self.handler.clone();
            cx.new(|cx| graph::GraphState::new(handler, cx))
        });
        entity.update(cx, |graph, cx| {
            if self.preview {
                graph.set_preview();
            }
            graph.set_snapshot(has_nodes.then_some(built), reset_view, cx);
        });
    }

    fn render_graph(&self) -> AnyElement {
        self.graph.as_ref().map_or_else(
            || div().into_any_element(),
            |graph| graph.clone().into_any_element(),
        )
    }

    fn render_view_switcher(
        &self,
        definition: &BaseDefinition,
        selected_name: Option<&str>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let active = selected_name
            .map(str::to_string)
            .or_else(|| definition.views.first().map(|view| view.name.clone()));
        if cfg!(any(target_os = "ios", target_os = "android")) && !self.preview {
            return self.touch_view_switcher(definition, active.as_deref(), cx);
        }
        h_flex()
            .gap_1()
            .px_2()
            .py_1()
            .border_b_1()
            .border_color(cx.theme().border)
            .children(definition.views.iter().map(|view| {
                let selected = active.as_deref() == Some(view.name.as_str());
                let name = view.name.clone();
                Button::new(ElementId::Name(format!("base-view-{name}").into()))
                    .ghost()
                    .small()
                    .label(view.name.clone())
                    .when(selected, ButtonVariants::primary)
                    .when(self.preview, |button| {
                        button
                            .bg(if selected {
                                cx.theme().primary
                            } else {
                                cx.theme().background
                            })
                            .text_color(if selected {
                                cx.theme().primary_foreground
                            } else {
                                cx.theme().foreground
                            })
                    })
                    .on_click(cx.listener(move |state, _, _, cx| {
                        if !state.preview {
                            state.select_view(name.clone(), cx);
                        }
                    }))
                    .into_any_element()
            }))
            .into_any_element()
    }

    fn touch_view_switcher(
        &self,
        definition: &BaseDefinition,
        active: Option<&str>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let current = active.unwrap_or("Views");
        // A table compares one property at a time; its column choice shares
        // the view's line so the two choosers cost one row.
        let table = definition
            .views
            .iter()
            .find(|v| Some(v.name.as_str()) == active)
            .filter(|v| v.view_type == ViewType::Table)
            .and_then(|v| Some((v, self.touch_rows.get(&v.name)?)));
        let view_trigger = touch_choice::trigger(
            "choose-base-view",
            current.to_owned(),
            "Choose view",
            self.choosing_view,
        )
        .flex_1()
        .min_w_0()
        .on_click(cx.listener(|state, _, _, cx| {
            state.choosing_view = !state.choosing_view;
            if let Some(rows) = state
                .selected_view
                .as_ref()
                .and_then(|name| state.touch_rows.get_mut(name))
            {
                rows.choosing_property = false;
            }
            cx.notify();
        }));
        let mut chooser = v_flex()
            .flex_shrink_0()
            .px_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(h_flex().w_full().gap_1().child(view_trigger).children(
                table.map(|(view, rows)| Self::property_picker(definition, view, rows, cx)),
            ));
        if let Some((view, rows)) = table.filter(|(_, rows)| rows.choosing_property) {
            chooser = chooser.child(Self::property_options(definition, view, rows.property, cx));
        }
        if self.choosing_view {
            let rows = touch_choice::options(
                "base-view-options",
                definition.views.iter().map(|view| {
                    let name = view.name.clone();
                    touch_choice::option(
                        ElementId::Name(format!("base-option-{name}").into()),
                        name.clone(),
                        active == Some(name.as_str()),
                    )
                    .on_click(cx.listener(move |state, _, _, cx| {
                        state.choosing_view = false;
                        if state.selected_view.as_deref() != Some(&name) {
                            state.select_view(name.clone(), cx);
                        }
                        cx.notify();
                    }))
                }),
            );
            chooser = chooser.child(rows);
        }
        chooser.into_any_element()
    }

    fn render_content(
        &self,
        snapshot: &BaseSnapshot,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(view) = snapshot.definition.views.get(snapshot.view_index) else {
            return centered_message("Base view is missing", cx);
        };
        if cfg!(any(target_os = "ios", target_os = "android"))
            && !self.preview
            && matches!(view.view_type, ViewType::List | ViewType::Table)
        {
            return self.render_touch_rows(snapshot, view, cx);
        }
        match view.view_type {
            ViewType::List => self.render_list(snapshot, view, cx),
            ViewType::Table => self.render_table(snapshot, view, window, cx),
            ViewType::Cards => self.render_cards(snapshot, view, window, cx),
            ViewType::Kanban => self.render_kanban(snapshot, view, cx),
            ViewType::Graph => self.render_graph(),
        }
    }
}

impl Render for BaseViewState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut column = v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .font_family(cx.theme().font_family.clone());
        if let Some(definition) = &self.definition {
            column = column.child(self.render_view_switcher(
                definition,
                self.selected_view.as_deref(),
                cx,
            ));
        }
        let content = match &self.status {
            BaseStatus::Loading => centered_message("Loading Base...", cx),
            BaseStatus::Error(error) => centered_message(error, cx),
            BaseStatus::Empty(_) => centered_message("No matching files", cx),
            BaseStatus::Ready(snapshot) => {
                let content = self.render_content(snapshot, window, cx);
                let notice = (snapshot.omitted > 0).then(|| {
                    div()
                        .px_2()
                        .py_1()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!(
                            "Showing {} of {} matching files",
                            snapshot.rows.len(),
                            snapshot.total
                        ))
                        .into_any_element()
                });
                v_flex()
                    .size_full()
                    .children(notice)
                    .child(content)
                    .into_any_element()
            }
        };
        let root_content = column.child(content).into_any_element();
        let scroll = self.shown_scroll_handle();
        let mut root = div()
            .id("base-viewer-root")
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .size_full()
            .relative()
            .overflow_hidden()
            .track_focus(&self.focus_handle)
            .key_context(crate::contexts::BASE_VIEWER_CONTEXT)
            .on_action(cx.listener(|this, _: &Cancel, _, cx| {
                if this.fullscreen_image.take().is_some() {
                    cx.notify();
                } else {
                    cx.propagate();
                }
            }))
            .on_action(cx.listener(|this, _: &NextView, _, cx| {
                if !this.preview {
                    this.select_adjacent_view(true, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &PreviousView, _, cx| {
                if !this.preview {
                    this.select_adjacent_view(false, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &ZoomIn, _, cx| {
                this.update_shown_graph(cx, |graph, cx| graph.zoom_by(ZOOM_STEP, cx));
            }))
            .on_action(cx.listener(|this, _: &ZoomOut, _, cx| {
                this.update_shown_graph(cx, |graph, cx| graph.zoom_by(ZOOM_STEP.recip(), cx));
            }))
            .on_action(cx.listener(|this, _: &ResetZoom, _, cx| {
                this.update_shown_graph(cx, graph::GraphState::reset_zoom);
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(cards::hide_fullscreen_image))
            .child(root_content);
        if let Some(preview) = cards::render_fullscreen_image(self.fullscreen_image.as_ref(), cx) {
            root = root.child(preview);
        }
        // An element has one key context, so the scrolling keys sit on a
        // wrapper, which the focused root dispatches through.
        if let Some(scroll) = scroll {
            return crate::scroll::scroll_with_keys(div().size_full(), &scroll)
                .child(root)
                .into_any_element();
        }
        if self.shown_graph().is_some() {
            return Self::with_graph_keys(div().size_full(), cx)
                .child(root)
                .into_any_element();
        }
        root.into_any_element()
    }
}

fn edited_value(
    original: Option<&serde_json::Value>,
    text: &str,
) -> anyhow::Result<Option<serde_json::Value>> {
    use serde_json::Value;
    if text.is_empty() {
        return Ok(None);
    }
    let value = match original {
        Some(Value::Number(_)) => Value::Number(
            text.trim()
                .parse()
                .map_err(|_| anyhow::anyhow!("Enter a valid number"))?,
        ),
        Some(Value::Bool(_)) => Value::Bool(
            text.trim()
                .parse()
                .map_err(|_| anyhow::anyhow!("Enter true or false"))?,
        ),
        Some(Value::Array(_) | Value::Object(_)) => {
            anyhow::bail!("This property requires a structured value")
        }
        _ => Value::String(text.to_owned()),
    };
    Ok(Some(value))
}

#[cfg(test)]
mod edit_tests {
    use super::edited_value;
    use serde_json::json;

    #[test]
    fn edits_preserve_scalar_types() {
        for (original, text, expected) in [
            (json!(5), "6", json!(6)),
            (json!("5"), "6", json!("6")),
            (json!(false), "true", json!(true)),
        ] {
            assert_eq!(edited_value(Some(&original), text).unwrap(), Some(expected));
        }
        assert!(edited_value(Some(&json!(5)), "unknown").is_err());
    }
}
