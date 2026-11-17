//! Kanban board: columns from the `groupBy` values, cards draggable by row.

use gpui_kit::component::scroll::{Scrollbar, ScrollbarMode};
use gpui_kit::component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};
use gpui_kit::{
    AnyElement, AppContext, Context, ElementId, Entity, InteractiveElement, IntoElement,
    ParentElement, Render, ScrollHandle, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px,
};

use crate::document::base::BaseView;
use crate::vault::source;

use super::cards;
use super::cells::{format_scalar_text, render_property_cell};
use super::snapshot::{BaseItem, BaseRow, BaseSnapshot, BaseStatus};
use super::{BaseViewState, centered_message, commit_property};

pub(super) struct KanbanState {
    pub(super) h_scroll_handle: ScrollHandle,
    column_scroll_handles: Vec<ScrollHandle>,
}

impl KanbanState {
    pub(super) fn new() -> Self {
        Self {
            h_scroll_handle: ScrollHandle::new(),
            column_scroll_handles: Vec::new(),
        }
    }

    pub(super) fn ensure_columns(&mut self, count: usize) {
        while self.column_scroll_handles.len() < count {
            self.column_scroll_handles.push(ScrollHandle::new());
        }
        self.column_scroll_handles.truncate(count);
    }
}

/// Drag payload for a card: the catalog-relative path of the dragged row.
#[derive(Clone)]
pub(super) struct DragCard {
    path: std::path::PathBuf,
}

impl Render for DragCard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let name = self
            .path
            .file_name()
            .map_or_default(|name| name.to_string_lossy().to_string());
        div()
            .px_2()
            .py_1()
            .bg(cx.theme().accent)
            .text_color(cx.theme().accent_foreground)
            .rounded_sm()
            .text_sm()
            .border_1()
            .border_color(cx.theme().border)
            .child(name)
    }
}

struct Column {
    label: String,
    count: usize,
    ordinal: usize,
    rows: Vec<usize>,
    /// Raw text of the group value (`""` when the column is Empty).
    raw: String,
    is_empty: bool,
}

fn build_columns(snapshot: &BaseSnapshot, view: &BaseView) -> Vec<Column> {
    let group_source = view
        .as_kanban()
        .map_or_default(|config| config.group.property.source.clone());
    let mut columns: Vec<Column> = Vec::new();
    for item in &snapshot.items {
        match item {
            BaseItem::Header {
                label,
                count,
                ordinal,
                ..
            } => {
                columns.push(Column {
                    label: label.clone(),
                    count: *count,
                    ordinal: *ordinal,
                    rows: Vec::new(),
                    raw: String::new(),
                    is_empty: false,
                });
            }
            BaseItem::Row { index, .. } => {
                if let Some(column) = columns.last_mut() {
                    column.rows.push(*index);
                }
            }
        }
    }
    // Each column's raw group value comes from its own first row.
    for column in &mut columns {
        let Some(row) = column
            .rows
            .first()
            .and_then(|index| snapshot.rows.get(*index))
        else {
            column.is_empty = column.label == "Empty";
            column.raw.clone_from(&column.label);
            continue;
        };
        let value = snapshot.projection_value(row, &group_source);
        column.is_empty = value
            .is_none_or(|value| value.is_null() || format_scalar_text(value).trim().is_empty());
        column.raw = value.map_or_default(format_scalar_text);
    }
    columns
}

fn is_folder_group(view: &BaseView) -> bool {
    view.as_kanban()
        .is_some_and(|config| config.group.property.source == "file.folder")
}

fn card_draggable(state: &BaseViewState, view: &BaseView, row: &BaseRow) -> bool {
    if state.preview {
        return false;
    }
    let Some(catalog) = state.catalog.as_ref() else {
        return false;
    };
    if source::is_read_only(&catalog.root().join(&row.path)) {
        return false;
    }
    if is_folder_group(view) {
        return true;
    }
    let Some(config) = view.as_kanban() else {
        return false;
    };
    let group_source = &config.group.property.source;
    if group_source.starts_with("file.") || group_source.starts_with("formula.") {
        return false;
    }
    catalog.has_yaml_frontmatter(&row.path)
}

impl BaseViewState {
    pub(super) fn render_kanban(
        &self,
        snapshot: &BaseSnapshot,
        view: &BaseView,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(kanban_state) = self.kanban.as_ref() else {
            return centered_message("Kanban view state is missing", cx);
        };
        let Some(config) = view.as_kanban().cloned() else {
            return centered_message("Kanban view config is missing", cx);
        };
        let column_width = config.column_width;
        let columns = build_columns(snapshot, view);
        let column_handles = kanban_state.column_scroll_handles.clone();
        let board = h_flex()
            .gap_3()
            .p_3()
            .h_full()
            .children(columns.iter().map(|column| {
                render_column(
                    column,
                    snapshot,
                    view,
                    &config,
                    column_width,
                    column_handles.get(column.ordinal),
                    self,
                    cx,
                )
            }))
            .into_any_element();
        div()
            .relative()
            .size_full()
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .id("base-kanban-scroll")
                    .size_full()
                    .overflow_x_scroll()
                    .track_scroll(&kanban_state.h_scroll_handle)
                    .child(board),
            )
            .child(
                div().absolute().inset_0().child(
                    Scrollbar::horizontal(&kanban_state.h_scroll_handle)
                        .mode(ScrollbarMode::Always)
                        .viewport_from_layout(),
                ),
            )
            .into_any_element()
    }

    fn drop_card_on_column(
        &mut self,
        dragged_path: &std::path::Path,
        raw: &str,
        is_empty: bool,
        cx: &mut Context<Self>,
    ) {
        if self.preview {
            return;
        }
        let snapshot = match &self.status {
            BaseStatus::Ready(snapshot) | BaseStatus::Empty(snapshot) => snapshot.clone(),
            _ => return,
        };
        let Some(row_index) = snapshot
            .rows
            .iter()
            .position(|row| row.path == dragged_path)
        else {
            return;
        };
        let Some(row) = snapshot.rows.get(row_index) else {
            return;
        };
        let Some(view) = snapshot.definition.views.get(snapshot.view_index).cloned() else {
            return;
        };
        let Some(catalog) = self.catalog.clone() else {
            return;
        };
        if source::is_read_only(&catalog.root().join(&row.path)) {
            return;
        }
        let result = if is_folder_group(&view) {
            let old_absolute = catalog.root().join(&row.path);
            let Some(file_name) = row.path.file_name().map(std::ffi::OsStr::to_os_string) else {
                return;
            };
            let destination_dir = catalog.root().join(raw);
            let destination =
                crate::vault::path::unique_path(&destination_dir, &file_name.to_string_lossy());
            if destination == old_absolute {
                return;
            }
            crate::vault::file_ops::rename(&catalog, &old_absolute, &destination).map(|_result| {
                catalog.refresh_paths(vec![old_absolute, destination]);
            })
        } else {
            if !catalog.has_yaml_frontmatter(&row.path) {
                return;
            }
            let Some(config) = view.as_kanban() else {
                return;
            };
            let group_source = &config.group.property.source;
            if group_source.starts_with("file.") || group_source.starts_with("formula.") {
                return;
            }
            let new_value = if is_empty { "" } else { raw };
            let absolute = catalog.root().join(&row.path);
            commit_property(&catalog, &row.path, group_source, new_value).map(|()| {
                catalog.refresh_paths(vec![absolute]);
            })
        };
        if result.is_ok() {
            self.rebuild(cx);
        } else {
            cx.notify();
        }
    }

    fn create_kanban_card(&mut self, raw: &str, is_empty: bool, cx: &mut Context<Self>) {
        if self.preview {
            return;
        }
        let snapshot = match &self.status {
            BaseStatus::Ready(snapshot) | BaseStatus::Empty(snapshot) => snapshot.clone(),
            _ => return,
        };
        let Some(view) = snapshot.definition.views.get(snapshot.view_index).cloned() else {
            return;
        };
        let Some(catalog) = self.catalog.clone() else {
            return;
        };
        let target_dir = if is_folder_group(&view) {
            catalog.root().join(raw)
        } else {
            let Some(config) = view.as_kanban() else {
                return;
            };
            let group_source = &config.group.property.source;
            if group_source.starts_with("file.") || group_source.starts_with("formula.") {
                return;
            }
            catalog.root()
        };
        let target = target_dir.join("New Note.md");
        let Ok(created) = crate::vault::file_ops::create_with_name(&target, "New Note.md") else {
            cx.notify();
            return;
        };
        if !is_folder_group(&view) && !is_empty {
            let group_source = view
                .as_kanban()
                .map_or_default(|config| config.group.property.source.clone());
            let _ = crate::vault::file_ops::update_frontmatter(&created, &group_source, Some(raw));
        }
        catalog.refresh_paths(vec![created]);
        self.rebuild(cx);
    }
}

#[allow(clippy::too_many_arguments)]
fn render_column(
    column: &Column,
    snapshot: &BaseSnapshot,
    view: &BaseView,
    config: &crate::document::base::KanbanConfig,
    column_width: f32,
    scroll_handle: Option<&ScrollHandle>,
    state: &BaseViewState,
    cx: &Context<BaseViewState>,
) -> AnyElement {
    let entity = cx.entity();
    let cards = column.rows.iter().map(|index| {
        render_card(
            *index,
            snapshot,
            view,
            config,
            column_width,
            &entity,
            state,
            cx,
        )
    });
    let cards_list = v_flex().gap_2().p_2().children(cards);
    let body = match scroll_handle {
        Some(handle) => div()
            .id(ElementId::Name(
                format!("base-kanban-scroll-{}", column.ordinal).into(),
            ))
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(handle)
            .child(cards_list)
            .into_any_element(),
        None => cards_list.flex_1().min_h_0().into_any_element(),
    };
    let raw = column.raw.clone();
    let drop_raw = raw.clone();
    let is_empty = column.is_empty;
    v_flex()
        .id(ElementId::Name(
            format!("base-kanban-column-{}", column.ordinal).into(),
        ))
        .w(px(column_width))
        .flex_shrink_0()
        .h_full()
        .rounded(px(6.))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary)
        .drag_over::<DragCard>(|style, _drag, _window, cx| style.bg(cx.theme().drop_target))
        .on_drop(cx.listener(move |state, drag: &DragCard, _window, cx| {
            state.drop_card_on_column(&drag.path, &drop_raw, is_empty, cx);
        }))
        .child(
            div()
                .px_3()
                .py_2()
                .text_color(cx.theme().muted_foreground)
                .child(format!("{} ({})", column.label, column.count)),
        )
        .child(body)
        .child(
            Button::new(ElementId::Name(
                format!("base-kanban-new-{}", column.ordinal).into(),
            ))
            .ghost()
            .small()
            .label("+ New")
            .on_click(cx.listener(move |state, _, _, cx| {
                state.create_kanban_card(&raw, is_empty, cx);
            }))
            .into_any_element(),
        )
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn render_card(
    index: usize,
    snapshot: &BaseSnapshot,
    view: &BaseView,
    config: &crate::document::base::KanbanConfig,
    column_width: f32,
    entity: &Entity<BaseViewState>,
    state: &BaseViewState,
    cx: &Context<BaseViewState>,
) -> AnyElement {
    let Some(row) = snapshot.rows.get(index) else {
        return div().into_any_element();
    };
    let card_width = (column_width - 16.0).max(120.0);
    let image = row
        .image
        .clone()
        .map(|image| cards::render_card_image(image, card_width, view, entity.clone(), cx));
    let image_source = config
        .image
        .as_ref()
        .map(|property| property.source.as_str());
    let mut properties = view
        .order
        .iter()
        .enumerate()
        .filter(|(_, property)| image_source != Some(property.source.as_str()))
        .map(|(column, property)| {
            let cell = render_property_cell(
                snapshot,
                row,
                property,
                &state.handler,
                ElementId::Name(format!("base-kanban-cell-{index}-{column}").into()),
                false,
                cx.theme(),
            );
            if column == 0 {
                div().w_full().text_lg().child(cell).into_any_element()
            } else {
                v_flex()
                    .w_full()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(snapshot.definition.display_label(property).to_string()),
                    )
                    .child(div().w_full().text_base().child(cell))
                    .into_any_element()
            }
        })
        .peekable();
    let has_properties = properties.peek().is_some();
    let card = v_flex()
        .id(ElementId::Name(format!("base-kanban-card-{index}").into()))
        .w_full()
        .flex_shrink_0()
        .overflow_hidden()
        .rounded(px(6.))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
        .children(image);
    // The drag source is the card body only:
    // pressing on the image must open it fullscreen.
    let mut body = v_flex()
        .id(ElementId::Name(
            format!("base-kanban-card-body-{index}").into(),
        ))
        .whitespace_normal()
        .gap_1()
        .p_3()
        .when(has_properties, |flex| flex.children(properties));
    if card_draggable(state, view, row) {
        body = body.on_drag(
            DragCard {
                path: row.path.clone(),
            },
            |drag, _offset, _window, cx| {
                cx.stop_propagation();
                cx.new(|_| drag.clone())
            },
        );
    }
    card.child(body).into_any_element()
}
