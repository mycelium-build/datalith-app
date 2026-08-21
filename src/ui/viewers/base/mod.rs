mod cards;
mod list;
mod table;

use std::cmp::Ordering;
use std::path::PathBuf;

use gpui::{
    AnyElement, App, AppContext, ClickEvent, Context, ElementId, Entity, FocusHandle,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Render,
    StatefulInteractiveElement, Styled, Task, Window, div, prelude::FluentBuilder,
};
use gpui_component::input::EditorState;
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};

use crate::document::base::{
    BaseDefinition, DisplayProperty, HARD_RESULT_LIMIT, SortDirection, SortRule, ViewType,
};
use crate::document::filter::{FileField, PropertyPath};
use crate::document::handler::{FileHandler, FileHandlerEvent};
use crate::vault::catalog::CatalogDocument;
use crate::vault::{CatalogQuery, VaultCatalog};

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

    pub(crate) fn refresh(&self, cx: &mut App) {
        self.state.update(cx, BaseViewState::rebuild);
    }

    pub(crate) fn set_vault_catalog(&self, catalog: VaultCatalog, cx: &mut Context<FileHandler>) {
        self.state.update(cx, |state, cx| {
            state.catalog = Some(catalog);
            state.rebuild(cx);
        });
    }

    pub(crate) fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.state.read(cx).focus_handle.clone()
    }

    pub(crate) fn render(&self, _handler: Entity<FileHandler>, _cx: &mut App) -> AnyElement {
        self.state.clone().into_any_element()
    }
}

#[derive(Clone, Debug)]
struct BaseRow {
    path: PathBuf,
    properties: yaml_serde::Value,
    size_bytes: i64,
    modified_ns: i64,
    links: Vec<String>,
    image: Option<cards::CardImage>,
}

#[derive(Clone, Debug)]
struct BaseSnapshot {
    definition: BaseDefinition,
    view_index: usize,
    rows: Vec<BaseRow>,
    total: usize,
    omitted: usize,
}

enum BaseStatus {
    Loading,
    Empty(BaseSnapshot),
    Ready(BaseSnapshot),
    Error(String),
}

pub struct BaseViewState {
    input: Entity<EditorState>,
    pub(crate) catalog: Option<VaultCatalog>,
    handler: gpui::WeakEntity<FileHandler>,
    status: BaseStatus,
    pub(crate) focus_handle: FocusHandle,
    list: list::ListState,
    table: table::TableState,
    cards: cards::CardsState,
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
            catalog,
            handler,
            status: BaseStatus::Loading,
            focus_handle: cx.focus_handle(),
            list: list::ListState::new(),
            table: table::TableState::new(),
            cards: cards::CardsState::new(),
            selected_view: None,
            generation: 0,
            build_task: Task::ready(()),
        }
    }

    fn rebuild(&mut self, cx: &mut Context<Self>) {
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.status = BaseStatus::Loading;
        self.list.item_sizes.clear();
        self.table.item_sizes.clear();
        let source = self.input.read(cx).value().to_string();
        let definition = match BaseDefinition::parse(&source) {
            Ok(definition) => definition,
            Err(error) => {
                self.status = BaseStatus::Error(error.to_string());
                cx.notify();
                return;
            }
        };
        let selected_name = self.selected_view.clone();
        let Some(catalog) = self.catalog.clone() else {
            self.status = BaseStatus::Error("No Vault Catalog is available".into());
            cx.notify();
            return;
        };
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
                        state.selected_view = snapshot
                            .definition
                            .views
                            .get(snapshot.view_index)
                            .map(|view| view.name.clone());
                        match snapshot.definition.views.get(snapshot.view_index) {
                            Some(view) if view.view_type == ViewType::List => {
                                state.list.item_sizes = list::row_sizes(&snapshot);
                            }
                            Some(view) if view.view_type == ViewType::Table => {
                                state.table.item_sizes = table::row_sizes(&snapshot);
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
        self.rebuild(cx);
    }

    fn render_view_switcher(snapshot: &BaseSnapshot, cx: &Context<Self>) -> AnyElement {
        h_flex()
            .gap_1()
            .px_2()
            .py_1()
            .border_b_1()
            .border_color(cx.theme().border)
            .children(
                snapshot
                    .definition
                    .views
                    .iter()
                    .enumerate()
                    .map(|(index, view)| {
                        let selected = index == snapshot.view_index;
                        let name = view.name.clone();
                        Button::new(ElementId::Name(format!("base-view-{index}").into()))
                            .ghost()
                            .small()
                            .label(view.name.clone())
                            .when(selected, ButtonVariants::primary)
                            .on_click(cx.listener(move |state, _, _, cx| {
                                state.select_view(name.clone(), cx);
                            }))
                            .into_any_element()
                    }),
            )
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
            ViewType::Table => self.render_table(snapshot, view, cx),
            ViewType::Cards => self.render_cards(snapshot, view, window, cx),
        }
    }
}

impl Render for BaseViewState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match &self.status {
            BaseStatus::Loading => centered_message("Loading Base...", cx),
            BaseStatus::Error(error) => centered_message(error, cx),
            BaseStatus::Empty(snapshot) => {
                let switcher = Self::render_view_switcher(snapshot, cx);
                v_flex()
                    .size_full()
                    .child(switcher)
                    .child(centered_message("No matching files", cx))
                    .into_any_element()
            }
            BaseStatus::Ready(snapshot) => {
                let switcher = Self::render_view_switcher(snapshot, cx);
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
                    .child(switcher)
                    .children(notice)
                    .child(content)
                    .into_any_element()
            }
        };
        let mut root = div()
            .size_full()
            .relative()
            .overflow_hidden()
            .on_mouse_up(MouseButton::Left, cx.listener(cards::hide_fullscreen_image))
            .child(content);
        if let Some(preview) = self.cards.render_fullscreen_image(cx) {
            root = root.child(preview);
        }
        root
    }
}

async fn load_snapshot(
    definition: BaseDefinition,
    selected_name: Option<&str>,
    catalog: VaultCatalog,
) -> anyhow::Result<BaseSnapshot> {
    let view_index = selected_name
        .and_then(|name| definition.views.iter().position(|view| view.name == name))
        .unwrap_or_default();
    let view = definition
        .views
        .get(view_index)
        .ok_or_else(|| anyhow::anyhow!("Base view is missing"))?;
    let selection = catalog
        .query_documents_with_outgoing_links(CatalogQuery {
            extension: None,
            filter: definition.catalog_filter(view),
            limit: None,
        })
        .await?;
    let root = catalog.root();
    let mut rows = selection
        .documents
        .into_iter()
        .filter_map(|document| base_row(document, &root))
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| compare_rows(left, right, &view.sort));
    let total = rows.len();
    let limit = view
        .limit
        .unwrap_or(HARD_RESULT_LIMIT)
        .min(HARD_RESULT_LIMIT);
    rows.truncate(limit);
    if view.view_type == ViewType::Cards
        && let Some(image) = &view.image
    {
        let root = catalog.root();
        for row in &mut rows {
            row.image = cards::resolve_card_image(&image.path, row, &catalog, &root);
        }
    }
    let omitted = total.saturating_sub(rows.len());
    Ok(BaseSnapshot {
        definition,
        view_index,
        rows,
        total,
        omitted,
    })
}

fn base_row(document: CatalogDocument, root: &std::path::Path) -> Option<BaseRow> {
    let path = document.path.strip_prefix(root).ok()?.to_path_buf();
    let properties = document.metadata.map_or_else(
        || yaml_serde::Value::Mapping(yaml_serde::Mapping::default()),
        |metadata| {
            yaml_serde::from_str(&metadata.to_string())
                .unwrap_or_else(|_| yaml_serde::Value::Mapping(yaml_serde::Mapping::default()))
        },
    );
    let links = document
        .links
        .into_iter()
        .filter_map(|link| link.strip_prefix(root).ok().map(path_text))
        .collect();
    Some(BaseRow {
        path,
        properties,
        size_bytes: document.size_bytes,
        modified_ns: document.modified_ns,
        links,
        image: None,
    })
}

fn path_text(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn compare_rows(left: &BaseRow, right: &BaseRow, rules: &[SortRule]) -> Ordering {
    for rule in rules {
        let ordering = compare_property(&rule.path, left, right);
        if ordering != Ordering::Equal {
            return match rule.direction {
                SortDirection::Asc => ordering,
                SortDirection::Desc => ordering.reverse(),
            };
        }
    }
    path_text(&left.path).cmp(&path_text(&right.path))
}

fn compare_property(path: &PropertyPath, left: &BaseRow, right: &BaseRow) -> Ordering {
    match path {
        PropertyPath::Note(parts) => compare_values(
            parts
                .iter()
                .try_fold(&left.properties, |value, part| value.get(part)),
            parts
                .iter()
                .try_fold(&right.properties, |value, part| value.get(part)),
        ),
        PropertyPath::File(field) => match field {
            FileField::Name => compare_text(file_name(&left.path), file_name(&right.path)),
            FileField::Ext => compare_text(
                left.path.extension().and_then(|value| value.to_str()),
                right.path.extension().and_then(|value| value.to_str()),
            ),
            FileField::Path => {
                compare_text(Some(&path_text(&left.path)), Some(&path_text(&right.path)))
            }
            FileField::Folder => compare_text(
                left.path.parent().map(path_text).as_deref(),
                right.path.parent().map(path_text).as_deref(),
            ),
            FileField::Size => left.size_bytes.cmp(&right.size_bytes),
            FileField::Mtime => left.modified_ns.cmp(&right.modified_ns),
            FileField::Links => left.links.len().cmp(&right.links.len()),
        },
    }
}

fn compare_text(left: Option<&str>, right: Option<&str>) -> Ordering {
    match (left, right) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(left), Some(right)) => left.cmp(right),
    }
}

fn compare_values(left: Option<&yaml_serde::Value>, right: Option<&yaml_serde::Value>) -> Ordering {
    match (left, right) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(left), Some(right)) => match (left, right) {
            (yaml_serde::Value::Number(left), yaml_serde::Value::Number(right)) => left
                .as_f64()
                .partial_cmp(&right.as_f64())
                .unwrap_or(Ordering::Equal),
            (yaml_serde::Value::Bool(left), yaml_serde::Value::Bool(right)) => left.cmp(right),
            (yaml_serde::Value::String(left), yaml_serde::Value::String(right)) => left.cmp(right),
            _ => value_text(left).cmp(&value_text(right)),
        },
    }
}

fn value_text(value: &yaml_serde::Value) -> String {
    match value {
        yaml_serde::Value::Null => String::new(),
        yaml_serde::Value::Bool(value) => value.to_string(),
        yaml_serde::Value::Number(value) => value.to_string(),
        yaml_serde::Value::String(value) => value.clone(),
        yaml_serde::Value::Sequence(values) => {
            values.iter().map(value_text).collect::<Vec<_>>().join(", ")
        }
        yaml_serde::Value::Mapping(_) | yaml_serde::Value::Tagged(_) => {
            yaml_serde::to_string(value)
                .unwrap_or_default()
                .trim()
                .to_string()
        }
    }
}

fn file_name(path: &std::path::Path) -> Option<&str> {
    path.file_stem().and_then(|value| value.to_str())
}

fn render_property_cell(
    row: &BaseRow,
    property: &DisplayProperty,
    handler: &gpui::WeakEntity<FileHandler>,
    row_index: usize,
    column_index: usize,
    truncate: bool,
    cx: &App,
) -> AnyElement {
    let id = ElementId::Name(format!("base-cell-{row_index}-{column_index}").into());
    if property.source == "file.name" {
        return render_link(
            id,
            file_name(&row.path).unwrap_or_default(),
            path_text(&row.path),
            handler.clone(),
            cx,
        );
    }
    if property.source == "file.links" {
        let links = row.links.iter().enumerate().map(|(index, target)| {
            render_link(
                ElementId::NamedInteger(
                    "base-link".into(),
                    u64::try_from(row_index.saturating_mul(1024).saturating_add(index))
                        .unwrap_or_default(),
                ),
                file_name(std::path::Path::new(target)).unwrap_or(target),
                target.clone(),
                handler.clone(),
                cx,
            )
        });
        return h_flex().gap_1().children(links).into_any_element();
    }
    if let Some((label, target)) = property_link(&property.path, row) {
        return render_link(id, &label, target, handler.clone(), cx);
    }
    let value = property_text(&property.path, row);
    div()
        .id(id)
        .when(truncate, Styled::text_ellipsis)
        .whitespace_normal()
        .child(value)
        .into_any_element()
}

fn property_value<'a>(path: &PropertyPath, row: &'a BaseRow) -> Option<&'a yaml_serde::Value> {
    match path {
        PropertyPath::Note(parts) => parts
            .iter()
            .try_fold(&row.properties, |value, part| value.get(part)),
        PropertyPath::File(_) => None,
    }
}

fn property_link(path: &PropertyPath, row: &BaseRow) -> Option<(String, String)> {
    let value = property_value(path, row)?;
    let value = value.as_str()?;
    let link = value.strip_prefix("[[")?.strip_suffix("]]")?;
    let (target, label) = link.split_once('|').unwrap_or((link, link));
    Some((label.to_string(), target.to_string()))
}

fn property_text(path: &PropertyPath, row: &BaseRow) -> String {
    match path {
        PropertyPath::Note(parts) => parts
            .iter()
            .try_fold(&row.properties, |value, part| value.get(part))
            .map_or_else(String::new, value_text),
        PropertyPath::File(field) => match field {
            FileField::Name => file_name(&row.path).unwrap_or_default().to_string(),
            FileField::Ext => row
                .path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_string(),
            FileField::Path => path_text(&row.path),
            FileField::Folder => row.path.parent().map(path_text).unwrap_or_default(),
            FileField::Size => format_size(row.size_bytes),
            FileField::Mtime => format_mtime(row.modified_ns),
            FileField::Links => row.links.join(", "),
        },
    }
}

fn format_size(bytes: i64) -> String {
    if bytes < 1_024 {
        return format!("{bytes} B");
    }
    let bytes = bytes.to_string().parse::<f64>().unwrap_or(0.0);
    if bytes < 1_048_576.0 {
        return format!("{:.1} KiB", bytes / 1_024.0);
    }
    if bytes < 1_073_741_824.0 {
        return format!("{:.1} MiB", bytes / 1_048_576.0);
    }
    format!("{:.1} GiB", bytes / 1_073_741_824.0)
}

fn format_mtime(modified_ns: i64) -> String {
    time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(modified_ns))
        .ok()
        .and_then(|time| {
            time.format(&time::format_description::well_known::Rfc3339)
                .ok()
        })
        .unwrap_or_default()
}

fn render_link(
    id: ElementId,
    label: &str,
    target: String,
    handler: gpui::WeakEntity<FileHandler>,
    cx: &App,
) -> AnyElement {
    div()
        .id(id)
        .text_color(cx.theme().primary)
        .hover(Styled::underline)
        .cursor_pointer()
        .on_click(move |event: &ClickEvent, _window, cx| {
            if let Some(handler) = handler.upgrade() {
                handler.update(cx, |_, cx| {
                    cx.emit(FileHandlerEvent::LinkClicked(
                        target.clone(),
                        event.modifiers().platform,
                    ));
                });
            }
        })
        .child(label.to_string())
        .into_any_element()
}

fn centered_message(message: &str, cx: &Context<BaseViewState>) -> AnyElement {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .text_color(cx.theme().muted_foreground)
        .child(message.to_string())
        .into_any_element()
}
