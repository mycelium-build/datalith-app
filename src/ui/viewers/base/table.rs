use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement, Pixels, Size, Styled, TextRun, Window,
    div, px, size,
};
use gpui_component::{ActiveTheme, VirtualListScrollHandle, h_flex, v_flex, v_virtual_list};
use gpui_component::{scroll::ScrollableElement, scroll::Scrollbar, scroll::ScrollbarMode};

use crate::document::base::{BaseView, TableRowHeight};

use super::{BaseRow, BaseSnapshot, BaseStatus, BaseViewState};

const TABLE_HEADER_HEIGHT: f32 = 32.0;
const TABLE_COLUMN_MIN_WIDTH: f32 = 128.0;
const TABLE_COLUMN_MAX_WIDTH: f32 = 512.0;
const TABLE_COLUMN_HORIZONTAL_PADDING: f32 = 16.0;
const TABLE_SHORT_HEIGHT: f32 = 24.0;
const TABLE_MEDIUM_HEIGHT: f32 = 32.0;
const TABLE_TALL_HEIGHT: f32 = 48.0;
const TABLE_EXTRA_TALL_HEIGHT: f32 = 72.0;

pub(super) struct TableState {
    pub(super) scroll_handle: VirtualListScrollHandle,
    pub(super) item_sizes: Vec<Size<Pixels>>,
}

impl TableState {
    pub(super) fn new() -> Self {
        Self {
            scroll_handle: VirtualListScrollHandle::new(),
            item_sizes: Vec::new(),
        }
    }
}

impl BaseViewState {
    pub(super) fn render_table(
        &self,
        snapshot: &BaseSnapshot,
        view: &BaseView,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(table_state) = self.table.as_ref() else {
            return super::centered_message("Table view state is missing", cx);
        };
        let entity = cx.entity();
        let item_sizes = table_state.item_sizes.clone();
        let handler = self.handler.clone();
        let definition = snapshot.definition.clone();
        let column_widths = column_widths(snapshot, view, window);
        let table_min_width = table_width(&column_widths);
        let row_column_widths = column_widths.clone();
        let header = h_flex()
            .w_full()
            .min_w(table_min_width)
            .h(px(TABLE_HEADER_HEIGHT))
            .bg(cx.theme().tab_bar)
            .border_b_1()
            .border_color(cx.theme().border)
            .children(view.order.iter().enumerate().map(|(column, property)| {
                div()
                    .w(column_widths[column])
                    .min_w(column_widths[column])
                    .max_w(px(TABLE_COLUMN_MAX_WIDTH))
                    .flex_shrink_0()
                    .px_2()
                    .items_center()
                    .text_color(cx.theme().muted_foreground)
                    .child(definition.display_name(property).to_string())
            }));
        let list = v_virtual_list(
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
                        render_table_row(
                            index,
                            row,
                            view,
                            table_min_width,
                            &row_column_widths,
                            &handler,
                            cx,
                        )
                    })
                    .collect()
            },
        )
        .track_scroll(&table_state.scroll_handle)
        .size_full();
        let body = div().relative().w_full().flex_1().min_h_0().child(list);
        div()
            .relative()
            .size_full()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .child(
                div().size_full().overflow_x_scrollbar().child(
                    v_flex()
                        .size_full()
                        .min_w(table_min_width)
                        .child(header)
                        .child(body),
                ),
            )
            .child(
                div().absolute().inset_0().child(
                    Scrollbar::vertical(&table_state.scroll_handle)
                        .mode(ScrollbarMode::Always)
                        .viewport_from_layout(),
                ),
            )
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
    table_min_width: Pixels,
    column_widths: &[Pixels],
    handler: &gpui::WeakEntity<crate::document::handler::FileHandler>,
    cx: &App,
) -> AnyElement {
    div()
        .w_full()
        .min_w(table_min_width)
        .h(px(table_row_height(view.row_height)))
        .flex()
        .items_center()
        .border_b_1()
        .border_color(cx.theme().border)
        .children(view.order.iter().enumerate().map(|(column, property)| {
            div()
                .w(column_widths[column])
                .min_w(column_widths[column])
                .max_w(px(TABLE_COLUMN_MAX_WIDTH))
                .flex_shrink_0()
                .px_2()
                .child(super::render_property_cell(
                    row, property, handler, index, column, true, cx,
                ))
        }))
        .into_any_element()
}

fn column_widths(snapshot: &BaseSnapshot, view: &BaseView, window: &Window) -> Vec<Pixels> {
    let text_style = window.text_style();
    let font_size = text_style.font_size.to_pixels(window.rem_size());
    view.order
        .iter()
        .map(|property| {
            let header_width = measure_text(
                &snapshot.definition.display_name(property).to_string(),
                window,
                &text_style,
                font_size,
            );
            let content_width = snapshot
                .rows
                .iter()
                .map(|row| {
                    measure_text(
                        &table_cell_text(property, row),
                        window,
                        &text_style,
                        font_size,
                    )
                })
                .fold(0.0, f32::max);
            px(
                (header_width.max(content_width) + TABLE_COLUMN_HORIZONTAL_PADDING)
                    .clamp(TABLE_COLUMN_MIN_WIDTH, TABLE_COLUMN_MAX_WIDTH),
            )
        })
        .collect()
}

fn table_width(column_widths: &[Pixels]) -> Pixels {
    px(column_widths.iter().map(|width| f32::from(*width)).sum())
}

fn measure_text(
    text: &str,
    window: &Window,
    text_style: &gpui::TextStyle,
    font_size: Pixels,
) -> f32 {
    text.split(|character| character == '\r' || character == '\n')
        .map(|line| {
            f32::from(
                window
                    .text_system()
                    .layout_line(
                        line,
                        font_size,
                        &[TextRun {
                            len: line.len(),
                            font: text_style.font(),
                            color: text_style.color,
                            ..Default::default()
                        }],
                        None,
                    )
                    .width,
            )
        })
        .fold(0.0, f32::max)
}

fn table_cell_text(property: &crate::document::base::DisplayProperty, row: &BaseRow) -> String {
    if property.source == "file.name" {
        return super::file_name(&row.path).unwrap_or_default().to_string();
    }
    if property.source == "file.links" {
        return row
            .links
            .iter()
            .map(|target| super::file_name(std::path::Path::new(target)).unwrap_or(target))
            .collect::<Vec<_>>()
            .join("  ");
    }
    if let Some((label, _)) = super::property_link(&property.path, row) {
        return label;
    }
    super::property_text(&property.path, row)
}
