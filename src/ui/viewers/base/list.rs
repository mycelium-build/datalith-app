use gpui_kit::component::{ActiveTheme, VirtualListScrollHandle, h_flex, v_flex, v_virtual_list};
use gpui_kit::component::{scroll::Scrollbar, scroll::ScrollbarMode};
use gpui_kit::{
    AnyElement, App, Context, ElementId, InteractiveElement, IntoElement, ParentElement, Pixels,
    Size, Styled, div, px, size,
};
use std::collections::HashMap;

use crate::document::base::{BaseView, ListMarkers};

use super::{BaseItem, BaseRow, BaseSnapshot, BaseStatus, BaseViewState};

const LIST_ROW_HEIGHT: f32 = 28.0;
const GROUP_HEADER_HEIGHT: f32 = 28.0;

/// One line of the list view: a group header, a row, or a Summary pseudo-row.
#[derive(Clone, Debug)]
pub(super) enum ListItem {
    Header { label: String, ordinal: usize },
    Row { index: usize, ordinal: usize },
    Summary { group: Option<String> },
}

pub(super) fn list_items(
    items: &[BaseItem],
    whole_set: &[super::snapshot::SummaryDisplay],
    group_summaries: &HashMap<String, Vec<super::snapshot::SummaryDisplay>>,
    grouped: bool,
) -> Vec<ListItem> {
    let mut result = Vec::with_capacity(items.len());
    if !grouped && !whole_set.is_empty() {
        result.push(ListItem::Summary { group: None });
    }
    for item in items {
        match item {
            BaseItem::Header { label, ordinal, .. } => {
                let has_entries = group_summaries
                    .get(label)
                    .is_some_and(|entries| !entries.is_empty());
                result.push(ListItem::Header {
                    label: label.clone(),
                    ordinal: *ordinal,
                });
                if has_entries {
                    result.push(ListItem::Summary {
                        group: Some(label.clone()),
                    });
                }
            }
            BaseItem::Row { index, ordinal } => result.push(ListItem::Row {
                index: *index,
                ordinal: *ordinal,
            }),
        }
    }
    result
}

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
        _snapshot: &BaseSnapshot,
        _view: &BaseView,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(list_state) = self.list.as_ref() else {
            return super::centered_message("List view state is missing", cx);
        };
        let entity = cx.entity();
        let item_sizes = list_state.item_sizes.clone();
        let handler = self.handler.clone();
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
                        let Some(item) = snapshot.list_items.get(index) else {
                            return div().into_any_element();
                        };
                        let markers = view.as_list().map_or(ListMarkers::Bullets, |l| l.markers);
                        match item {
                            ListItem::Header { label, ordinal } => render_group_header(
                                format!("base-list-header-{index}"),
                                label,
                                *ordinal,
                                markers,
                                cx,
                            ),
                            ListItem::Row {
                                index: row_index,
                                ordinal,
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
                            ListItem::Summary { group } => render_summary_item(
                                format!("base-list-summary-{index}"),
                                group.as_deref(),
                                snapshot,
                                markers,
                                cx,
                            ),
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
            .into_any_element()
    }
}

pub(super) fn row_sizes(snapshot: &BaseSnapshot) -> Vec<Size<Pixels>> {
    let Some(view) = snapshot.definition.views.get(snapshot.view_index) else {
        return Vec::new();
    };
    let height = list_row_height(view);
    snapshot
        .list_items
        .iter()
        .map(|item| match item {
            ListItem::Header { .. } => size(px(1.), px(GROUP_HEADER_HEIGHT)),
            ListItem::Row { .. } => size(px(1.), px(height)),
            ListItem::Summary { group } => {
                let entries = summary_item_entries(group.as_deref(), snapshot);
                let lines = u16::try_from(entries.len().saturating_add(1)).unwrap_or(u16::MAX);
                size(px(1.), px(LIST_ROW_HEIGHT * f32::from(lines)))
            }
        })
        .collect()
}

/// Summary displays for a Summary pseudo-row: the whole set, or one group's.
fn summary_item_entries<'a>(
    group: Option<&str>,
    snapshot: &'a BaseSnapshot,
) -> &'a [super::snapshot::SummaryDisplay] {
    group.map_or(&snapshot.summaries[..], |label| {
        snapshot.group_summaries_for(label)
    })
}

fn list_row_height(view: &BaseView) -> f32 {
    let list = view.as_list().cloned().unwrap_or_default();
    if !list.indent_properties {
        return LIST_ROW_HEIGHT;
    }
    let lines = u16::try_from(view.order.len().max(1)).unwrap_or(u16::MAX);
    LIST_ROW_HEIGHT * f32::from(lines)
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
    v_flex()
        .id(ElementId::Name(id.into()))
        .h(px(GROUP_HEADER_HEIGHT))
        .justify_center()
        .child(
            h_flex()
                .items_center()
                .child(marker)
                .child(label.to_string()),
        )
        .into_any_element()
}

/// The "Summary" pseudo-row: one bullet line, then one nested line per entry.
fn render_summary_item(
    id: String,
    group: Option<&str>,
    snapshot: &BaseSnapshot,
    markers: ListMarkers,
    _cx: &App,
) -> AnyElement {
    let entries = summary_item_entries(group, snapshot);
    let top_marker = match markers {
        ListMarkers::Bullets => "• ".to_string(),
        ListMarkers::Numbers | ListMarkers::None => String::new(),
    };
    let sub_marker = match markers {
        ListMarkers::None => String::new(),
        _ => "• ".to_string(),
    };
    let entry_lines = entries
        .iter()
        .map(|display| {
            h_flex()
                .items_center()
                .h(px(LIST_ROW_HEIGHT))
                .child(sub_marker.clone())
                .child(super::snapshot::summary_entry_text(display))
                .into_any_element()
        })
        .collect::<Vec<_>>();
    let mut column = gpui_kit::component::v_flex()
        .id(ElementId::Name(id.into()))
        .w_full()
        .child(
            h_flex()
                .items_center()
                .h(px(LIST_ROW_HEIGHT))
                .child(top_marker)
                .child("Summary"),
        )
        .child(gpui_kit::component::v_flex().pl_4().children(entry_lines));
    if group.is_some() {
        column = column.pl_4();
    }
    column.into_any_element()
}

fn render_list_row(
    ordinal: usize,
    snapshot: &BaseSnapshot,
    row: &BaseRow,
    definition: &crate::document::base::BaseDefinition,
    view: &BaseView,
    handler: &gpui_kit::WeakEntity<crate::document::handler::FileHandler>,
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
            gpui_kit::component::v_flex()
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
    let mut container = gpui_kit::component::v_flex().w_full();
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
    handler: &gpui_kit::WeakEntity<crate::document::handler::FileHandler>,
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

#[cfg(test)]
mod tests {
    use super::super::snapshot::SummaryDisplay;
    use super::{BaseItem, ListItem, list_items};
    use std::collections::HashMap;

    fn display(label: &str) -> SummaryDisplay {
        SummaryDisplay {
            source: String::new(),
            label: label.to_string(),
            title: "Sum".to_string(),
            text: "1".to_string(),
        }
    }

    fn header(label: &str) -> BaseItem {
        BaseItem::Header {
            label: label.to_string(),
            count: 1,
            ordinal: 0,
        }
    }

    fn row(index: usize) -> BaseItem {
        BaseItem::Row {
            index,
            ordinal: index,
        }
    }

    #[test]
    fn ungrouped_lists_get_one_whole_set_summary_item() {
        let items = vec![row(0), row(1)];
        let whole = vec![display("Pages")];
        let items = list_items(&items, &whole, &HashMap::new(), false);
        assert!(matches!(&items[0], ListItem::Summary { group: None }));
        assert!(matches!(&items[1], ListItem::Row { index: 0, .. }));

        let items = vec![row(0)];
        let items = list_items(&items, &[], &HashMap::new(), false);
        assert!(
            matches!(&items[0], ListItem::Row { .. }),
            "no summaries, no item"
        );
    }

    #[test]
    fn grouped_lists_nest_a_summary_item_after_each_header() {
        let items = vec![header("done"), row(0), header("reading"), row(1), row(2)];
        let mut groups = HashMap::new();
        groups.insert("done".to_string(), vec![display("Pages")]);
        // "reading" has only null values, so it gets no map entry and no item.
        let items = list_items(&items, &[], &groups, true);
        assert!(matches!(&items[0], ListItem::Header { label, .. } if label == "done"));
        assert!(matches!(&items[1], ListItem::Summary { group: Some(label) } if label == "done"));
        assert!(matches!(&items[2], ListItem::Row { .. }));
        assert!(matches!(&items[3], ListItem::Header { label, .. } if label == "reading"));
        assert!(
            matches!(&items[4], ListItem::Row { .. }),
            "empty group has no summary item"
        );
    }
}
