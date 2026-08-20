use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement, Pixels, Size, Styled, div, px, size,
};
use gpui_component::{ActiveTheme, h_flex, v_flex, v_virtual_list};

use crate::document::base::{BaseView, TableRowHeight};

use super::{BaseRow, BaseSnapshot, BaseStatus, BaseViewState};

const TABLE_HEADER_HEIGHT: f32 = 32.0;
const TABLE_SHORT_HEIGHT: f32 = 24.0;
const TABLE_MEDIUM_HEIGHT: f32 = 32.0;
const TABLE_TALL_HEIGHT: f32 = 48.0;
const TABLE_EXTRA_TALL_HEIGHT: f32 = 72.0;

impl BaseViewState {
    pub(super) fn render_table(
        &self,
        snapshot: &BaseSnapshot,
        view: &BaseView,
        cx: &Context<Self>,
    ) -> AnyElement {
        let entity = cx.entity();
        let item_sizes = self.item_sizes.clone();
        let handler = self.handler.clone();
        let definition = snapshot.definition.clone();
        let header = h_flex()
            .w_full()
            .h(px(TABLE_HEADER_HEIGHT))
            .bg(cx.theme().tab_bar)
            .border_b_1()
            .border_color(cx.theme().border)
            .children(view.order.iter().map(|property| {
                div()
                    .flex_1()
                    .min_w_0()
                    .px_2()
                    .items_center()
                    .text_color(cx.theme().muted_foreground)
                    .child(definition.display_name(property).to_string())
            }));
        let body = v_virtual_list(
            entity,
            "base-table",
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
                        render_table_row(index, row, view, &handler, cx)
                    })
                    .collect()
            },
        );
        v_flex()
            .size_full()
            .child(header)
            .child(body)
            .into_any_element()
    }
}

pub(super) fn row_sizes(snapshot: &BaseSnapshot) -> Vec<Size<Pixels>> {
    let Some(view) = snapshot.definition.views.get(snapshot.view_index) else {
        return Vec::new();
    };
    let height = table_row_height(view.row_height);
    vec![size(px(1.), px(height)); snapshot.rows.len()]
}

const fn table_row_height(height: TableRowHeight) -> f32 {
    match height {
        TableRowHeight::Short => TABLE_SHORT_HEIGHT,
        TableRowHeight::Medium => TABLE_MEDIUM_HEIGHT,
        TableRowHeight::Tall => TABLE_TALL_HEIGHT,
        TableRowHeight::ExtraTall => TABLE_EXTRA_TALL_HEIGHT,
    }
}

fn render_table_row(
    index: usize,
    row: &BaseRow,
    view: &BaseView,
    handler: &gpui::WeakEntity<crate::document::handler::FileHandler>,
    cx: &App,
) -> AnyElement {
    div()
        .w_full()
        .h(px(table_row_height(view.row_height)))
        .flex()
        .items_center()
        .border_b_1()
        .border_color(cx.theme().border)
        .children(view.order.iter().enumerate().map(|(column, property)| {
            div()
                .flex_1()
                .min_w_0()
                .px_2()
                .child(super::render_property_cell(
                    row, property, handler, index, column, cx,
                ))
        }))
        .into_any_element()
}
