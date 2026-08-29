mod cards;
mod cells;
mod graph;
mod list;
mod snapshot;
mod table;

use cells::centered_message;
use cells::{format_scalar_text, render_property_cell};
use snapshot::file_name;
use snapshot::load_snapshot;
use snapshot::{BaseItem, BaseRow, BaseSnapshot, BaseStatus};

use gpui::{
    AnyElement, App, AppContext, Context, ElementId, Entity, FocusHandle, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Render, Styled, Task, Window, div,
    prelude::FluentBuilder,
};
use gpui_component::input::EditorState;
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};

use crate::document::base::{BaseDefinition, ViewType};
use crate::document::handler::FileHandler;
use crate::vault::VaultCatalog;

pub struct BaseViewer {
    state: Entity<BaseViewState>,
}

impl BaseViewer {
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
    input: Entity<EditorState>,
    /// The parsed definition the switcher renders from and queries reuse;
    /// replaced whenever the source parses successfully again.
    definition: Option<BaseDefinition>,
    pub(super) catalog: Option<VaultCatalog>,
    handler: gpui::WeakEntity<FileHandler>,
    status: BaseStatus,
    pub(super) focus_handle: FocusHandle,
    list: Option<list::ListState>,
    table: Option<table::TableState>,
    cards: Option<cards::CardsState>,
    graph: Option<Entity<graph::GraphState>>,
    /// The definition source the last successful build came from;
    /// graph camera/simulation reset only when this changes.
    last_source: String,
    selected_view: Option<String>,
    generation: u64,
    build_task: Task<()>,
}

impl BaseViewState {
    fn new(
        input: Entity<EditorState>,
        catalog: Option<VaultCatalog>,
        handler: gpui::WeakEntity<FileHandler>,
        cx: &Context<Self>,
    ) -> Self {
        Self {
            input,
            definition: None,
            catalog,
            handler,
            status: BaseStatus::Loading,
            focus_handle: cx.focus_handle(),
            list: None,
            table: None,
            cards: None,
            graph: None,
            last_source: String::new(),
            selected_view: None,
            generation: 0,
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
                    Ok(snapshot) => {
                        state.last_source.clone_from(&source);
                        state.selected_view = snapshot
                            .definition
                            .views
                            .get(snapshot.view_index)
                            .map(|view| view.name.clone());
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

    fn select_view(&mut self, name: String, cx: &mut Context<Self>) {
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
        view: &crate::document::base::BaseView,
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
        definition: &BaseDefinition,
        selected_name: Option<&str>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let active = selected_name
            .map(str::to_string)
            .or_else(|| definition.views.first().map(|view| view.name.clone()));
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
                    .on_click(cx.listener(move |state, _, _, cx| {
                        state.select_view(name.clone(), cx);
                    }))
                    .into_any_element()
            }))
            .into_any_element()
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
        match view.view_type {
            ViewType::List => self.render_list(snapshot, view, cx),
            ViewType::Table => self.render_table(snapshot, view, window, cx),
            ViewType::Cards => self.render_cards(snapshot, view, window, cx),
            ViewType::Graph => self.render_graph(),
        }
    }
}

impl Render for BaseViewState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut column = v_flex().size_full();
        if let Some(definition) = &self.definition {
            column = column.child(Self::render_view_switcher(
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
        let mut root = div()
            .id("base-viewer-root")
            .size_full()
            .relative()
            .overflow_hidden()
            .on_mouse_up(MouseButton::Left, cx.listener(cards::hide_fullscreen_image))
            .child(root_content);
        if let Some(preview) = self
            .cards
            .as_ref()
            .and_then(|cards| cards.render_fullscreen_image(cx))
        {
            root = root.child(preview);
        }
        root
    }
}
