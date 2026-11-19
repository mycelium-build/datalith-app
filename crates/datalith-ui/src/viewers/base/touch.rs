use super::{
    BaseItem, BaseRow, BaseSnapshot, BaseStatus, BaseViewState, snapshot::summary_inline_line,
    table::table_cell_text, touch_choice,
};
use crate::handler::FileHandlerEvent;
use datalith_core::base::{BaseDefinition, BaseView, ViewType};
use gpui_kit::component::{
    ActiveTheme, ElementExt, IconName,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled, UniformListScrollHandle, div, px, uniform_list,
};
use std::path::PathBuf;

#[derive(Default)]
pub(super) struct Rows {
    scroll: UniformListScrollHandle,
    pub(super) property: usize,
    extra_columns: usize,
    pub(super) detail: Option<PathBuf>,
    pub(super) choosing_property: bool,
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
        let selected = rows
            .detail
            .as_ref()
            .and_then(|p| snap.rows.iter().find(|r| r.path == *p));
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
                    .child(div().flex_1().min_w_0().truncate().child(
                        view.order.get(rows.property).map_or_else(String::new, |p| {
                            snap.definition.display_label(p).to_string()
                        }),
                    ))
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
        // The selected row's sheet sits under the table, which stays visible
        // and scrollable above it.
        layout
            .child(list)
            .children(
                summary_inline_line(&snap.summaries)
                    .map(|s| div().px_3().text_sm().truncate().child(s)),
            )
            .children(selected.map(|row| self.touch_detail(snap, view, row, cx)))
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
                let selected = rows.detail.as_ref() == Some(&path);
                div()
                    .id(gpui_kit::SharedString::from(format!(
                        "base-result:{}",
                        path.display()
                    )))
                    .w_full()
                    .h(px(56.))
                    .px_3()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .when(selected, |el| el.bg(cx.theme().selection))
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

    /// The compared column, beside the view trigger on the same line.
    pub(super) fn property_picker(
        definition: &BaseDefinition,
        view: &BaseView,
        rows: &Rows,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let value = view
            .order
            .get(rows.property)
            .map_or_else(String::new, |p| definition.display_label(p).to_string());
        let key = view.name.clone();
        touch_choice::trigger(
            "compare-property",
            value,
            "Choose property to compare",
            rows.choosing_property,
        )
        .flex_1()
        .min_w_0()
        .on_click(cx.listener(move |s, _, _, cx| {
            s.choosing_view = false;
            let rows = s.touch_rows.entry(key.clone()).or_default();
            rows.choosing_property = !rows.choosing_property;
            cx.notify();
        }))
    }

    pub(super) fn property_options(
        definition: &BaseDefinition,
        view: &BaseView,
        active: usize,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        touch_choice::options(
            "compare-property-options",
            view.order.iter().enumerate().map(|(ix, p)| {
                let key = view.name.clone();
                touch_choice::option(
                    gpui_kit::SharedString::from(format!("compare-property:{}", p.source)),
                    definition.display_label(p).to_string(),
                    ix == active,
                )
                .on_click(cx.listener(move |s, _, _, cx| {
                    let rows = s.touch_rows.entry(key.clone()).or_default();
                    rows.property = ix;
                    rows.choosing_property = false;
                    cx.notify();
                }))
            }),
        )
        .border_b_1()
        .border_color(cx.theme().border)
    }

    /// The bottom sheet for one table row: the note's name and folder, an
    /// explicit Open note action, Close, then every property of the view.
    /// Back closes it first (see `dismiss_control`).
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
        let title = super::file_name(&path).unwrap_or_default().to_string();
        let folder = path
            .parent()
            .map(|p| p.display().to_string())
            .filter(|p| !p.is_empty());
        let header = h_flex()
            .gap_2()
            .px_3()
            .pt_2()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .truncate()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .children(folder.map(|f| {
                        div()
                            .text_sm()
                            .truncate()
                            .text_color(cx.theme().muted_foreground)
                            .child(f)
                    })),
            )
            .child(
                Button::new("open-row")
                    .primary()
                    .h(px(44.))
                    .label("Open note")
                    .on_click(move |_, _, cx| {
                        let _ = handler.update(cx, |_, cx| {
                            cx.emit(FileHandlerEvent::VaultFileRequested(path.clone(), false));
                        });
                    }),
            )
            .child(
                Button::new("dismiss-row")
                    .ghost()
                    .w(px(44.))
                    .h(px(44.))
                    .icon(IconName::Close)
                    .accessibility_label("Close")
                    .on_click(cx.listener(move |s, _, _, cx| {
                        s.touch_rows.entry(key.clone()).or_default().detail = None;
                        cx.notify();
                    })),
            );
        v_flex()
            .id("row-sheet")
            .flex_shrink_0()
            .max_h(gpui_kit::relative(0.5))
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover)
            .rounded_t(cx.theme().radius_lg)
            .shadow_lg()
            .child(header)
            .child(
                v_flex()
                    .id("row-properties")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .pb_2()
                    .children(
                        view.order
                            .iter()
                            .enumerate()
                            .filter(|(_, p)| p.source != "file.name")
                            .map(|(ix, p)| {
                                v_flex()
                                    .px_3()
                                    .py_1()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(
                                                snapshot.definition.display_label(p).to_string(),
                                            ),
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
                            }),
                    ),
            )
            .into_any_element()
    }
}
