use super::{
    BaseItem, BaseRow, BaseSnapshot, BaseStatus, BaseViewState, snapshot::summary_inline_line,
    table::table_cell_text,
};
use crate::handler::FileHandlerEvent;
use datalith_core::base::{BaseView, ViewType};
use gpui_kit::component::{
    ActiveTheme, ElementExt, IconName,
    button::{Button, ButtonVariants},
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex,
};
use gpui_kit::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled, UniformListScrollHandle, div, px, uniform_list,
};
use std::path::PathBuf;

#[derive(Default)]
pub(super) struct Rows {
    scroll: UniformListScrollHandle,
    property: usize,
    extra_columns: usize,
    pub(super) detail: Option<PathBuf>,
}

#[derive(Default)]
pub(super) struct SavedView {
    cards: Option<super::cards::CardsState>,
    board: Option<super::kanban::KanbanState>,
    graph: Option<gpui_kit::Entity<super::graph::GraphState>>,
}

fn other_columns(
    view: &BaseView,
    active: usize,
    limit: usize,
) -> impl Iterator<Item = &datalith_core::base::DisplayProperty> {
    view.order
        .iter()
        .enumerate()
        .filter(move |(i, p)| *i != active && p.source != "file.name")
        .map(|(_, p)| p)
        .take(limit)
}
impl BaseViewState {
    pub(super) fn restore_mobile_view(&mut self, snapshot: &BaseSnapshot) {
        if !cfg!(any(target_os = "android", target_os = "ios")) {
            return;
        }
        let Some(view) = snapshot.definition.views.get(snapshot.view_index) else {
            return;
        };
        if self.active_state.as_ref() == Some(&view.name) {
            return;
        }
        if let Some(previous) = self.active_state.replace(view.name.clone()) {
            self.view_states.insert(
                previous,
                SavedView {
                    cards: self.cards.take(),
                    board: self.kanban.take(),
                    graph: self.graph.take(),
                },
            );
        }
        self.view_states
            .retain(|name, _| snapshot.definition.views.iter().any(|v| v.name == *name));
        self.touch_rows
            .retain(|name, _| snapshot.definition.views.iter().any(|v| v.name == *name));
        let saved = self.view_states.remove(&view.name).unwrap_or_default();
        self.cards = saved.cards;
        self.kanban = saved.board;
        self.graph = saved.graph;
    }

    pub(super) fn render_touch_rows(
        &self,
        snap: &BaseSnapshot,
        view: &BaseView,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(rows) = self.touch_rows.get(&view.name) else {
            return div().into_any_element();
        };
        if let Some(row) = rows
            .detail
            .as_ref()
            .and_then(|p| snap.rows.iter().find(|r| r.path == *p))
        {
            return self.touch_detail(snap, view, row, cx);
        }
        let table = view.view_type == ViewType::Table;
        let mut layout = v_flex().size_full().min_h_0();
        if table {
            layout = Self::measure_table(layout, &view.name, rows.extra_columns, cx);
            layout = layout.child(
                h_flex()
                    .px_2()
                    .w_full()
                    .gap_3()
                    .child(
                        div()
                            .w(gpui_kit::relative(0.44))
                            .flex_shrink_0()
                            .child("File"),
                    )
                    .child(Self::property_picker(snap, view, rows.property, cx))
                    .children(
                        other_columns(view, rows.property, rows.extra_columns).map(|p| {
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(snap.definition.display_label(p).to_string())
                        }),
                    ),
            );
        }
        let list = uniform_list(
            format!("base-rows-{}", view.name),
            snap.items.len(),
            cx.processor(|state, range: std::ops::Range<usize>, _, cx| {
                let BaseStatus::Ready(snapshot) = &state.status else {
                    return Vec::new();
                };
                let Some(view) = snapshot.definition.views.get(snapshot.view_index) else {
                    return Vec::new();
                };
                range
                    .filter_map(|ix| state.touch_result(snapshot, view, ix, cx))
                    .collect()
            }),
        )
        .track_scroll(&rows.scroll)
        .flex_1();
        layout
            .child(list)
            .children(
                summary_inline_line(&snap.summaries)
                    .map(|s| div().px_3().text_sm().truncate().child(s)),
            )
            .into_any_element()
    }

    fn measure_table(
        layout: gpui_kit::Div,
        name: &str,
        previous: usize,
        cx: &Context<Self>,
    ) -> gpui_kit::Div {
        let key = name.to_owned();
        let entity = cx.weak_entity();
        layout.on_prepaint(move |bounds, w, cx| {
            let count = if bounds.size.width >= px(800.) {
                2
            } else {
                usize::from(bounds.size.width >= px(560.))
            };
            if count == previous {
                return;
            }
            w.defer(cx, move |_, cx| {
                let _ = entity.update(cx, |s, cx| {
                    if let Some(rows) = s.touch_rows.get_mut(&key)
                        && rows.extra_columns != count
                    {
                        rows.extra_columns = count;
                        cx.notify();
                    }
                });
            });
        })
    }
    fn touch_result(
        &self,
        snapshot: &BaseSnapshot,
        view: &BaseView,
        ix: usize,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let table = view.view_type == ViewType::Table;
        let rows = self.touch_rows.get(&view.name)?;
        let property = rows.property;
        Some(match snapshot.items.get(ix)? {
            BaseItem::Header { label, count, .. } => {
                let summary = summary_inline_line(snapshot.group_summaries_for(label));
                v_flex()
                    .justify_center()
                    .px_3()
                    .h(px(56.))
                    .bg(cx.theme().secondary)
                    .child(format!("{label} ({count})"))
                    .children(summary.map(|v| div().w_full().truncate().text_sm().child(v)))
                    .into_any_element()
            }
            BaseItem::Row { index, .. } => {
                let row = snapshot.rows.get(*index)?;
                let path = row.path.clone();
                let name = super::file_name(&path).unwrap_or_default().to_string();
                let value = if table {
                    view.order
                        .get(property)
                        .map_or_default(|p| table_cell_text(snapshot, p, row))
                } else {
                    view.order
                        .iter()
                        .filter(|p| p.source != "file.name")
                        .map(|p| table_cell_text(snapshot, p, row))
                        .filter(|v| v != &name && !v.is_empty())
                        .collect::<Vec<_>>()
                        .join(" · ")
                };
                let title = div().min_w_0().truncate().child(name);
                let field = div().flex_1().min_w_0().truncate().child(value);
                let body = if table {
                    h_flex()
                        .w_full()
                        .gap_3()
                        .child(title.w(gpui_kit::relative(0.44)).flex_shrink_0())
                        .child(field)
                        .children(other_columns(view, property, rows.extra_columns).map(|p| {
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(table_cell_text(snapshot, p, row))
                        }))
                        .into_any_element()
                } else {
                    v_flex()
                        .w_full()
                        .child(title)
                        .child(field)
                        .into_any_element()
                };
                let entity = cx.entity();
                let key = view.name.clone();
                let handler = self.handler.clone();
                div()
                    .id(("base-result", ix))
                    .w_full()
                    .h(px(56.))
                    .px_3()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(body)
                    .on_click(move |_, _, cx| {
                        if table {
                            entity.update(cx, |state, cx| {
                                state.touch_rows.entry(key.clone()).or_default().detail =
                                    Some(path.clone());
                                cx.notify();
                            });
                        } else {
                            let _ = handler.update(cx, |_, cx| {
                                cx.emit(FileHandlerEvent::VaultFileRequested(path.clone(), false));
                            });
                        }
                    })
                    .into_any_element()
            }
        })
    }

    fn property_picker(
        snap: &BaseSnapshot,
        view: &BaseView,
        active: usize,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let names: Vec<_> = view
            .order
            .iter()
            .map(|p| snap.definition.display_label(p).to_string())
            .collect();
        let key = view.name.clone();
        let state = cx.weak_entity();
        Button::new("compare-property")
            .ghost()
            .h(px(48.))
            .flex_1()
            .label(names.get(active).cloned().unwrap_or_default())
            .dropdown_caret(true)
            .accessibility_label("Choose property to compare")
            .dropdown_menu(move |mut menu, _, _| {
                for (ix, name) in names.iter().enumerate() {
                    let state = state.clone();
                    let key = key.clone();
                    menu = menu.item(
                        PopupMenuItem::new(name.clone())
                            .checked(ix == active)
                            .on_click(move |_, _, cx| {
                                let _ = state.update(cx, |s, cx| {
                                    s.touch_rows.entry(key.clone()).or_default().property = ix;
                                    cx.notify();
                                });
                            }),
                    );
                }
                menu
            })
    }

    fn touch_detail(
        &self,
        snapshot: &BaseSnapshot,
        view: &BaseView,
        row: &BaseRow,
        cx: &Context<Self>,
    ) -> AnyElement {
        let key = view.name.clone();
        let path = row.path.clone();
        let handler = self.handler.clone();
        v_flex()
            .size_full()
            .child(
                h_flex()
                    .child(
                        Button::new("dismiss-row")
                            .ghost()
                            .h(px(48.))
                            .icon(IconName::ChevronLeft)
                            .accessibility_label("Back to table")
                            .on_click(cx.listener(move |s, _, _, cx| {
                                s.touch_rows.entry(key.clone()).or_default().detail = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .truncate()
                            .child(path.to_string_lossy().into_owned()),
                    )
                    .child(
                        Button::new("open-row")
                            .ghost()
                            .h(px(48.))
                            .icon(IconName::ArrowRight)
                            .accessibility_label("Open note")
                            .on_click(move |_, _, cx| {
                                let _ = handler.update(cx, |_, cx| {
                                    cx.emit(FileHandlerEvent::VaultFileRequested(
                                        path.clone(),
                                        false,
                                    ));
                                });
                            }),
                    ),
            )
            .child(
                v_flex()
                    .id("row-properties")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(view.order.iter().enumerate().map(|(ix, p)| {
                        v_flex()
                            .px_3()
                            .py_2()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(snapshot.definition.display_label(p).to_string()),
                            )
                            .child(super::render_property_cell(
                                snapshot,
                                row,
                                p,
                                &self.handler,
                                ("detail-property", ix).into(),
                                false,
                                cx.theme(),
                            ))
                    })),
            )
            .into_any_element()
    }
}
