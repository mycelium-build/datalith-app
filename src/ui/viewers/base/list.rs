use gpui::{
    AnyElement, App, Context, ElementId, InteractiveElement, IntoElement, ParentElement, Pixels,
    Size, Styled, div, px, size,
};
use gpui_component::{ActiveTheme, VirtualListScrollHandle, h_flex, v_virtual_list};
use gpui_component::{scroll::Scrollbar, scroll::ScrollbarMode};

use crate::document::base::{BaseView, ListMarkers};

use super::{BaseItem, BaseRow, BaseSnapshot, BaseStatus, BaseViewState};

const LIST_ROW_HEIGHT: f32 = 28.0;
const GROUP_HEADER_HEIGHT: f32 = 28.0;

pub(super) struct ListState {
    pub(super) scroll_handle: VirtualListScrollHandle,
    pub(super) item_sizes: Vec<Size<Pixels>>,
}

impl ListState {
    pub(super) fn new() -> Self {
        Self {
            scroll_handle: VirtualListScrollHandle::new(),
            item_sizes: Vec::new(),
        }
    }
}

impl BaseViewState {
    pub(super) fn render_list(
        &self,
        snapshot: &BaseSnapshot,
        _view: &BaseView,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(list_state) = self.list.as_ref() else {
            return super::centered_message("List view state is missing", cx);
        };
        let entity = cx.entity();
        let item_sizes = list_state.item_sizes.clone();
        let handler = self.handler.clone();
        let summary_strip = Self::render_summary_strip(snapshot, cx);
        let list = v_virtual_list(
            entity,
            "base-list",
            item_sizes.into(),
            move |state, visible_range, _, cx| {
                let (BaseStatus::Ready(snapshot) | BaseStatus::Empty(snapshot)) = &state.status
                else {
                    return Vec::new();
                };
                let Some(view) = snapshot.definition.views.get(snapshot.view_index) else {
                    return Vec::new();
                };
                visible_range
                    .map(|index| {
                        let Some(item) = snapshot.items.get(index) else {
                            return div().into_any_element();
                        };
                        match item {
                            BaseItem::Header { label, ordinal, .. } => {
                                let markers =
                                    view.as_list().map_or(ListMarkers::Bullets, |l| l.markers);
                                render_group_header(
                                    format!("base-list-header-{index}"),
                                    label,
                                    *ordinal,
                                    markers,
                                    cx,
                                )
                            }
                            BaseItem::Row {
                                index: row_index,
                                ordinal,
                                ..
                            } => {
                                let Some(row) = snapshot.rows.get(*row_index) else {
                                    return div().into_any_element();
                                };
                                render_list_row(
                                    *ordinal,
                                    snapshot,
                                    row,
                                    &snapshot.definition,
                                    view,
                                    &handler,
                                    cx,
                                )
                            }
                        }
                    })
                    .collect()
            },
        )
        .track_scroll(&list_state.scroll_handle)
        .size_full();
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .child(div().size_full().px_2().py_1().child(list))
            .child(
                div().absolute().inset_0().child(
                    Scrollbar::vertical(&list_state.scroll_handle)
                        .mode(ScrollbarMode::Always)
                        .viewport_from_layout(),
                ),
            )
            .children(summary_strip)
            .into_any_element()
    }
}

pub(super) fn row_sizes(snapshot: &BaseSnapshot) -> Vec<Size<Pixels>> {
    let Some(view) = snapshot.definition.views.get(snapshot.view_index) else {
        return Vec::new();
    };
    let height = list_row_height(view);
    snapshot
        .items
        .iter()
        .map(|item| match item {
            BaseItem::Header { .. } => size(px(1.), px(GROUP_HEADER_HEIGHT)),
            BaseItem::Row { .. } => size(px(1.), px(height)),
        })
        .collect()
}

fn list_row_height(view: &BaseView) -> f32 {
    let list = view.as_list().cloned().unwrap_or_default();
    if !list.indent_properties {
        return LIST_ROW_HEIGHT;
    }
    let lines = u32::try_from(view.order.len().max(1)).unwrap_or(u32::MAX);
    LIST_ROW_HEIGHT * lines.to_string().parse::<f32>().unwrap_or(1.0)
}

fn render_group_header(
    id: String,
    label: &str,
    ordinal: usize,
    markers: ListMarkers,
    _cx: &App,
) -> AnyElement {
    let marker = match markers {
        ListMarkers::Bullets => "• ".to_string(),
        ListMarkers::Numbers => format!("{}. ", ordinal.saturating_add(1)),
        ListMarkers::None => String::new(),
    };
    h_flex()
        .id(ElementId::Name(id.into()))
        .items_center()
        .h(px(GROUP_HEADER_HEIGHT))
        .child(marker)
        .child(label.to_string())
        .into_any_element()
}

fn render_list_row(
    ordinal: usize,
    snapshot: &BaseSnapshot,
    row: &BaseRow,
    definition: &crate::document::base::BaseDefinition,
    view: &BaseView,
    handler: &gpui::WeakEntity<crate::document::handler::FileHandler>,
    cx: &App,
) -> AnyElement {
    let list = view.as_list().cloned().unwrap_or_default();
    let marker = match list.markers {
        ListMarkers::Bullets => "• ".to_string(),
        ListMarkers::Numbers => format!("{}. ", ordinal.saturating_add(1)),
        ListMarkers::None => String::new(),
    };
    let mut lines = Vec::new();
    if list.indent_properties {
        if let Some(property) = view.order.first() {
            lines.push(
                h_flex()
                    .items_center()
                    .h(px(LIST_ROW_HEIGHT))
                    .child(marker)
                    .child(cell(snapshot, row, property, handler, 0, 0, true, cx))
                    .into_any_element(),
            );
        }
        let sub_marker = match list.markers {
            ListMarkers::None => String::new(),
            _ => "• ".to_string(),
        };
        let sub_items = view
            .order
            .iter()
            .enumerate()
            .skip(1)
            .map(|(column, property)| {
                h_flex()
                    .items_center()
                    .h(px(LIST_ROW_HEIGHT))
                    .gap_1()
                    .child(sub_marker.clone())
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{}:", definition.display_label(property))),
                    )
                    .child(cell(
                        snapshot, row, property, handler, ordinal, column, true, cx,
                    ))
                    .into_any_element()
            });
        lines.push(
            gpui_component::v_flex()
                .pl_4()
                .children(sub_items)
                .into_any_element(),
        );
    } else {
        let mut cells = Vec::new();
        for column in 0..view.order.len() {
            if column > 0 {
                cells.push(div().child(list.separators.clone()).into_any_element());
            }
            let Some(property) = view.order.get(column) else {
                continue;
            };
            cells.push(cell(
                snapshot, row, property, handler, ordinal, column, true, cx,
            ));
        }
        lines.push(
            h_flex()
                .items_center()
                .h(px(LIST_ROW_HEIGHT))
                .child(marker)
                .children(cells)
                .into_any_element(),
        );
    }
    let mut container = gpui_component::v_flex().w_full();
    if view.group_by.is_some() {
        container = container.pl_4();
    }
    container.children(lines).into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn cell(
    snapshot: &BaseSnapshot,
    row: &BaseRow,
    property: &crate::document::base::DisplayProperty,
    handler: &gpui::WeakEntity<crate::document::handler::FileHandler>,
    ordinal: usize,
    column: usize,
    truncate: bool,
    cx: &App,
) -> AnyElement {
    super::render_property_cell(
        snapshot,
        row,
        property,
        handler,
        ElementId::Name(format!("base-cell-{ordinal}-{column}").into()),
        truncate,
        cx,
    )
}
