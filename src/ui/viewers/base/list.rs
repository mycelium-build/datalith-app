use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement, Pixels, Size, Styled, div, px, size,
};
use gpui_component::{h_flex, v_virtual_list};

use crate::document::base::{BaseView, ListMarkers};

use super::{BaseDefinition, BaseRow, BaseSnapshot, BaseStatus, BaseViewState};

const LIST_ROW_HEIGHT: f32 = 28.0;

impl BaseViewState {
    pub(super) fn render_list(
        &self,
        _snapshot: &BaseSnapshot,
        _view: &BaseView,
        cx: &Context<Self>,
    ) -> AnyElement {
        let entity = cx.entity();
        let item_sizes = self.item_sizes.clone();
        let handler = self.handler.clone();
        v_virtual_list(
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
                        let Some(row) = snapshot.rows.get(index) else {
                            return div().into_any_element();
                        };
                        render_list_row(index, row, &snapshot.definition, view, &handler, cx)
                    })
                    .collect()
            },
        )
        .into_any_element()
    }
}

pub(super) fn row_sizes(snapshot: &BaseSnapshot) -> Vec<Size<Pixels>> {
    let Some(view) = snapshot.definition.views.get(snapshot.view_index) else {
        return Vec::new();
    };
    snapshot
        .rows
        .iter()
        .map(|_| size(px(1.), px(list_row_height(view))))
        .collect()
}

fn list_row_height(view: &BaseView) -> f32 {
    if !view.indent_properties {
        return LIST_ROW_HEIGHT;
    }
    let lines = u32::try_from(view.order.len().max(1)).unwrap_or(u32::MAX);
    LIST_ROW_HEIGHT * lines.to_string().parse::<f32>().unwrap_or(1.0)
}

fn render_list_row(
    index: usize,
    row: &BaseRow,
    definition: &BaseDefinition,
    view: &BaseView,
    handler: &gpui::WeakEntity<crate::document::handler::FileHandler>,
    cx: &App,
) -> AnyElement {
    let marker = match view.markers {
        ListMarkers::Bullets => "• ".to_string(),
        ListMarkers::Numbers => format!("{}. ", index.saturating_add(1)),
        ListMarkers::None => String::new(),
    };
    let mut lines = Vec::new();
    if view.indent_properties {
        if let Some(property) = view.order.first() {
            lines.push(
                h_flex()
                    .items_center()
                    .h(px(LIST_ROW_HEIGHT))
                    .child(marker)
                    .child(super::render_property_cell(
                        row, property, handler, index, 0, cx,
                    ))
                    .into_any_element(),
            );
        }
        lines.extend(
            view.order
                .iter()
                .enumerate()
                .skip(1)
                .map(|(column, property)| {
                    h_flex()
                        .items_center()
                        .h(px(LIST_ROW_HEIGHT))
                        .pl_4()
                        .child(definition.display_name(property).to_string())
                        .child(" ".to_string())
                        .child(super::render_property_cell(
                            row, property, handler, index, column, cx,
                        ))
                        .into_any_element()
                }),
        );
    } else {
        let mut cells = Vec::new();
        for (column, property) in view.order.iter().enumerate() {
            if column > 0 {
                cells.push(div().child(view.separators.clone()).into_any_element());
            }
            cells.push(super::render_property_cell(
                row, property, handler, index, column, cx,
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
    gpui_component::v_flex()
        .w_full()
        .children(lines)
        .into_any_element()
}
