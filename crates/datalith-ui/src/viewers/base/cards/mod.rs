use gpui_kit::component::scroll::{ScrollableElement, Scrollbar, ScrollbarMode};
use gpui_kit::component::{
    ActiveTheme, ElementExt, VirtualListScrollHandle, h_flex, v_flex, v_virtual_list,
};
use gpui_kit::{
    AnyElement, App, Context, ElementId, Entity, InteractiveElement, IntoElement, ObjectFit,
    ParentElement, Pixels, SharedUri, Size, Styled, Window, div, img, prelude::StyledImage as _,
    px, size,
};
use std::collections::HashMap;

use datalith_core::base::{BaseView, CardBodyHeight, DEFAULT_BODY_HEIGHT};

use super::{BaseItem, BaseRow, BaseSnapshot, BaseStatus, BaseViewState};

const CARD_RADIUS: f32 = 6.0;
const CARD_GAP: f32 = 16.0;
const CARD_MIN_WIDTH: f32 = 120.0;
const GRID_PADDING: f32 = 16.0;

mod image;

pub(super) use image::{
    CardImage, collect_card_image_targets, render_card_image, resolve_card_image,
};

pub(super) struct CardsState {
    pub(super) scroll_handle: VirtualListScrollHandle,
    pub(super) viewport_width: Pixels,
    pub(super) auto_body_heights: HashMap<usize, f32>,
    pub(super) auto_width: f32,
}

impl CardsState {
    pub(super) fn new() -> Self {
        Self {
            scroll_handle: VirtualListScrollHandle::new(),
            viewport_width: px(0.),
            auto_body_heights: HashMap::new(),
            auto_width: 0.0,
        }
    }
}

pub(super) fn render_fullscreen_image(image: Option<&CardImage>, cx: &App) -> Option<AnyElement> {
    let image = image?;
    let content = match image {
        CardImage::Local(path) => img(crate::viewers::image::image_source(path))
            .size_full()
            .object_fit(ObjectFit::Contain)
            .into_any_element(),
        CardImage::External(url) => img(SharedUri::from(url.clone()))
            .size_full()
            .object_fit(ObjectFit::Contain)
            .into_any_element(),
    };
    Some(
        div()
            .absolute()
            .inset_0()
            .size_full()
            .bg(cx.theme().background)
            .flex()
            .items_center()
            .justify_center()
            .child(content)
            .into_any_element(),
    )
}

pub(super) fn hide_fullscreen_image(
    state: &mut BaseViewState,
    _event: &gpui_kit::MouseUpEvent,
    _window: &mut Window,
    cx: &mut Context<BaseViewState>,
) {
    if state.fullscreen_image.take().is_some() {
        cx.notify();
    }
}

struct CardRenderContext<'a> {
    snapshot: &'a BaseSnapshot,
    view: &'a BaseView,
    handler: &'a gpui_kit::WeakEntity<crate::handler::FileHandler>,
    fullscreen_entity: &'a Entity<BaseViewState>,
    cx: &'a App,
}

impl BaseViewState {
    #[allow(clippy::too_many_lines)]
    pub(super) fn render_cards(
        &self,
        snapshot: &BaseSnapshot,
        view: &BaseView,
        _window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let cards_config = view.as_cards().cloned().unwrap_or_default();
        let Some(cards_state) = self.cards.as_ref() else {
            return super::centered_message("Cards view state is missing", cx);
        };
        let entity = cx.entity();
        // Empty until on_prepaint measures the viewport width.
        let viewport_width = cards_state.viewport_width;
        let list = if viewport_width == px(0.) {
            None
        } else {
            let columns = columns_for(cards_config.card_size, viewport_width);
            let card_width = card_width_for(cards_config.card_size, viewport_width, columns);
            let measured = same_card_width(cards_state.auto_width, card_width)
                .then_some(&cards_state.auto_body_heights);
            let item_sizes = card_row_sizes(snapshot, columns, card_width, measured);
            let handler = self.handler.clone();
            let fullscreen_entity = entity.clone();
            let list_entity = entity.clone();
            Some(
                v_virtual_list(
                    list_entity,
                    "base-cards",
                    item_sizes.into(),
                    move |state, visible_range, _, cx| {
                        let (BaseStatus::Ready(snapshot) | BaseStatus::Empty(snapshot)) =
                            &state.status
                        else {
                            return Vec::new();
                        };
                        let Some(view) = snapshot.definition.views.get(snapshot.view_index) else {
                            return Vec::new();
                        };
                        let context = CardRenderContext {
                            snapshot,
                            view,
                            handler: &handler,
                            fullscreen_entity: &fullscreen_entity,
                            cx,
                        };
                        let items = flatten_card_items(
                            &snapshot.items,
                            !snapshot.summaries.is_empty(),
                            columns,
                        );
                        visible_range
                            .filter_map(|item_index| {
                                let item = items.get(item_index)?;
                                Some(match item {
                                    CardItem::GlobalSummary => {
                                        render_global_summary(&snapshot.summaries, cx)
                                    }
                                    CardItem::Header { label, count } => render_group_header(
                                        label,
                                        *count,
                                        snapshot.group_summaries_for(label),
                                        cx,
                                    ),
                                    CardItem::GridRow(indices) => {
                                        render_card_row(indices, item_index, card_width, &context)
                                    }
                                })
                            })
                            .collect()
                    },
                )
                .track_scroll(&cards_state.scroll_handle)
                .size_full(),
            )
        };
        let viewport_entity = entity;
        let mut viewport = div()
            .id("base-cards-viewport")
            .relative()
            .flex_1()
            .min_h_0()
            .on_prepaint(move |bounds, window, cx| {
                let width = content_width(bounds.size.width);
                let mut changed = false;
                viewport_entity.update(cx, |state, _| {
                    if let Some(cards) = state.cards.as_mut()
                        && cards.viewport_width != width
                    {
                        cards.viewport_width = width;
                        changed = true;
                    }
                });
                if changed {
                    // Notifications during prepaint are discarded; defer past the frame.
                    window.defer(cx, {
                        let viewport_entity = viewport_entity.clone();
                        move |_, cx| viewport_entity.update(cx, |_, cx| cx.notify())
                    });
                }
            });
        if let Some(list) = list {
            viewport = viewport.child(div().size_full().p_4().child(list)).child(
                div().absolute().inset_0().child(
                    Scrollbar::vertical(&cards_state.scroll_handle)
                        .mode(ScrollbarMode::Always)
                        .viewport_from_layout(),
                ),
            );
        }
        viewport.into_any_element()
    }
}

/// One virtualized line of the cards layout: the global summary, a group header, or a grid row.
enum CardItem {
    GlobalSummary,
    Header { label: String, count: usize },
    GridRow(Vec<usize>),
}

fn flatten_card_items(items: &[BaseItem], has_summaries: bool, columns: usize) -> Vec<CardItem> {
    let mut result = Vec::new();
    if has_summaries {
        result.push(CardItem::GlobalSummary);
    }
    let mut pending: Vec<usize> = Vec::new();
    for item in items {
        match item {
            BaseItem::Header { label, count, .. } => {
                if !pending.is_empty() {
                    result.push(CardItem::GridRow(std::mem::take(&mut pending)));
                }
                result.push(CardItem::Header {
                    label: label.clone(),
                    count: *count,
                });
            }
            BaseItem::Row { index, .. } => {
                pending.push(*index);
                if pending.len() >= columns {
                    result.push(CardItem::GridRow(std::mem::take(&mut pending)));
                }
            }
        }
    }
    if !pending.is_empty() {
        result.push(CardItem::GridRow(pending));
    }
    result
}

#[allow(clippy::arithmetic_side_effects)]
fn content_width(width: Pixels) -> Pixels {
    let padding = px(GRID_PADDING * 2.0);
    if width > padding {
        width - padding
    } else {
        px(0.0)
    }
}

fn columns_for(card_size: f32, viewport_width: Pixels) -> usize {
    let width = f32::from(viewport_width).max(CARD_MIN_WIDTH);
    let card_size = card_size.max(CARD_MIN_WIDTH);
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::as_conversions
    )]
    let columns = ((width + CARD_GAP) / (card_size + CARD_GAP)).floor() as usize;
    columns.max(1)
}

fn card_width_for(card_size: f32, viewport_width: Pixels, columns: usize) -> f32 {
    let width = f32::from(viewport_width);
    if width <= 0.0 {
        return card_size;
    }
    let gaps = CARD_GAP * f32::from(u16::try_from(columns.saturating_sub(1)).unwrap_or(0));
    let columns = f32::from(u16::try_from(columns.max(1)).unwrap_or(1));
    ((width - gaps) / columns).max(CARD_MIN_WIDTH)
}

fn card_row_sizes(
    snapshot: &BaseSnapshot,
    columns: usize,
    card_width: f32,
    measured: Option<&HashMap<usize, f32>>,
) -> Vec<Size<Pixels>> {
    let Some(view) = snapshot.definition.views.get(snapshot.view_index) else {
        return Vec::new();
    };
    let image_height = view
        .as_cards()
        .and_then(|cards| Some((cards, cards.image.as_ref()?)))
        .map_or(0.0, |(cards, _)| card_width / cards.image_aspect_ratio);
    flatten_card_items(&snapshot.items, !snapshot.summaries.is_empty(), columns)
        .iter()
        .map(|item| match item {
            CardItem::GlobalSummary => size(px(1.0), px(GLOBAL_SUMMARY_HEIGHT)),
            CardItem::Header { label, .. } => size(
                px(1.0),
                px(if snapshot.group_summaries_for(label).is_empty() {
                    GROUP_HEADER_HEIGHT
                } else {
                    GROUP_HEADER_WITH_SUMMARIES_HEIGHT
                }),
            ),
            CardItem::GridRow(indices) => {
                let body = body_height_for_row(view, indices, measured);
                size(px(1.0), px(image_height + body + CARD_GAP))
            }
        })
        .collect()
}

const GLOBAL_SUMMARY_HEIGHT: f32 = 32.0;
const GROUP_HEADER_HEIGHT: f32 = 36.0;
const GROUP_HEADER_WITH_SUMMARIES_HEIGHT: f32 = 52.0;

/// The whole-set summary header above the grid, scrolling with the content.
fn render_global_summary(summaries: &[super::snapshot::SummaryDisplay], cx: &App) -> AnyElement {
    let Some(line) = super::snapshot::summary_inline_line(summaries) else {
        return div().into_any_element();
    };
    h_flex()
        .id(ElementId::Name("base-cards-global-summary".into()))
        .h(px(GLOBAL_SUMMARY_HEIGHT))
        .items_center()
        .text_color(cx.theme().muted_foreground)
        .child(line)
        .into_any_element()
}

fn render_group_header(
    label: &str,
    count: usize,
    summaries: &[super::snapshot::SummaryDisplay],
    cx: &App,
) -> AnyElement {
    let mut header = v_flex()
        .id(ElementId::Name("base-cards-group-header".into()))
        .h(px(if summaries.is_empty() {
            GROUP_HEADER_HEIGHT
        } else {
            GROUP_HEADER_WITH_SUMMARIES_HEIGHT
        }))
        .justify_center()
        .child(
            h_flex()
                .items_center()
                .gap_2()
                .text_color(cx.theme().muted_foreground)
                .child(format!("{label} ({count})")),
        );
    if let Some(line) = super::snapshot::summary_inline_line(summaries) {
        header = header.child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(line),
        );
    }
    header.into_any_element()
}

fn render_card_row(
    indices: &[usize],
    grid_row_index: usize,
    card_width: f32,
    context: &CardRenderContext<'_>,
) -> AnyElement {
    let cards = indices.iter().enumerate().map(|(slot, index)| {
        render_card(
            format!("base-card-{grid_row_index}-{slot}"),
            *index,
            card_width,
            context.snapshot.rows.get(*index),
            context,
        )
    });
    h_flex()
        .w_full()
        .items_start()
        .gap(px(CARD_GAP))
        .children(cards)
        .into_any_element()
}

#[allow(clippy::too_many_lines)]
fn render_card(
    id: String,
    index: usize,
    card_width: f32,
    row: Option<&BaseRow>,
    context: &CardRenderContext<'_>,
) -> AnyElement {
    let Some(row) = row else {
        return div().w(px(card_width)).into_any_element();
    };
    let card_id = ElementId::Name(id.into());
    let image = row.image.clone().map(|image| {
        image::render_card_image(
            image,
            card_width,
            context.view,
            context.fullscreen_entity.clone(),
            context.cx,
        )
    });
    let image_source = context
        .view
        .as_cards()
        .and_then(|cards| cards.image.as_ref())
        .map(|property| property.source.as_str());
    let properties = context
        .view
        .order
        .iter()
        .enumerate()
        .filter(|(_, property)| image_source != Some(property.source.as_str()))
        .enumerate()
        .map(|(column, property)| {
            let property = property.1;
            let cell = super::render_property_cell(
                context.snapshot,
                row,
                property,
                context.handler,
                ElementId::Name(format!("base-card-cell-{index}-{column}").into()),
                false,
                context.cx.theme(),
            );
            if property.source == "file.name" {
                div().w_full().text_lg().child(cell).into_any_element()
            } else {
                v_flex()
                    .w_full()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        div()
                            .text_xs()
                            .text_color(context.cx.theme().muted_foreground)
                            .child(
                                context
                                    .snapshot
                                    .definition
                                    .display_label(property)
                                    .to_string(),
                            ),
                    )
                    .child(div().w_full().text_base().child(cell))
                    .into_any_element()
            }
        });
    let body_content = v_flex()
        .whitespace_normal()
        .gap_1()
        .p_3()
        .children(properties);
    let is_auto = cfg!(any(target_os = "ios", target_os = "android"))
        || matches!(
            context.view.as_cards().map(|cards| cards.body_height),
            Some(CardBodyHeight::Auto)
        );
    let body = if is_auto {
        let entity = context.fullscreen_entity.clone();
        div()
            .child(body_content.on_prepaint(move |bounds, window, cx| {
                let height = f32::from(bounds.size.height);
                let mut changed = false;
                entity.update(cx, |state, _| {
                    if let Some(cards) = state.cards.as_mut() {
                        if !same_card_width(cards.auto_width, card_width) {
                            cards.auto_width = card_width;
                            cards.auto_body_heights.clear();
                        }
                        let stored = cards.auto_body_heights.get(&index).copied();
                        if stored.is_none_or(|stored| (stored - height).abs() > 0.5) {
                            cards.auto_body_heights.insert(index, height);
                            changed = true;
                        }
                    }
                });
                if changed {
                    window.defer(cx, {
                        let entity = entity.clone();
                        move |_, cx| entity.update(cx, |_, cx| cx.notify())
                    });
                }
            }))
            .into_any_element()
    } else {
        div()
            .flex_1()
            .min_h_0()
            .overflow_y_scrollbar()
            .child(body_content)
            .into_any_element()
    };
    let card = v_flex().id(card_id).w(px(card_width));
    let card = if is_auto {
        card
    } else {
        card.h(px(card_height(context.view, card_width)))
    };
    let card = if cfg!(any(target_os = "ios", target_os = "android")) {
        use gpui_kit::StatefulInteractiveElement as _;
        let path = row.path.clone();
        let handler = context.handler.clone();
        card.on_click(move |_, _, cx| {
            let _ = handler.update(cx, |_, cx| {
                cx.emit(crate::handler::FileHandlerEvent::VaultFileRequested(
                    path.clone(),
                    false,
                ));
            });
            cx.stop_propagation();
        })
    } else {
        card
    };
    card.flex_shrink_0()
        .overflow_hidden()
        .rounded(px(CARD_RADIUS))
        .border_1()
        .border_color(context.cx.theme().border)
        .bg(context.cx.theme().secondary)
        .children(image)
        .child(body)
        .into_any_element()
}

fn same_card_width(stored: f32, current: f32) -> bool {
    (stored - current).abs() < 0.5
}

/// Below-image body height for one grid row:
/// fixed size or auto (tallest measured body)
fn body_height_for_row(
    view: &BaseView,
    indices: &[usize],
    measured: Option<&HashMap<usize, f32>>,
) -> f32 {
    let config = view.as_cards().cloned().unwrap_or_default();
    match if cfg!(any(target_os = "android", target_os = "ios")) {
        CardBodyHeight::Auto
    } else {
        config.body_height
    } {
        CardBodyHeight::Fixed(height) => height,
        CardBodyHeight::Auto => {
            let measured_max = indices
                .iter()
                .filter_map(|index| measured.and_then(|heights| heights.get(index)).copied())
                .fold(0.0, f32::max);
            if measured_max > 0.0 {
                measured_max
            } else {
                DEFAULT_BODY_HEIGHT
            }
        }
    }
}

fn card_height(view: &BaseView, card_width: f32) -> f32 {
    let body = match view.as_cards().map(|cards| cards.body_height) {
        Some(CardBodyHeight::Fixed(height)) => height,
        _ => DEFAULT_BODY_HEIGHT,
    };
    let image_height = view
        .as_cards()
        .and_then(|cards| Some((cards, cards.image.as_ref()?)))
        .map_or(0.0, |(cards, _)| card_width / cards.image_aspect_ratio);
    image_height + body
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use super::*;
    use datalith_core::base::{CardBodyHeight, CardsConfig, SortDirection, ViewKind, ViewType};
    use datalith_core::filter::{Filter, PropertyPath};

    fn auto_view() -> BaseView {
        BaseView {
            view_type: ViewType::Cards,
            name: "Gallery".to_string(),
            filters: Filter::MatchAll,
            limit: None,
            order: vec![datalith_core::base::DisplayProperty {
                source: "note".to_string(),
                path: PropertyPath::Note(vec!["note".to_string()]),
            }],
            sort: Vec::new(),
            group_by: None,
            group_direction: SortDirection::Asc,
            summaries: BTreeMap::new(),
            kind: ViewKind::Cards(CardsConfig {
                body_height: CardBodyHeight::Auto,
                ..Default::default()
            }),
        }
    }

    #[test]
    fn auto_row_height_prefers_measured_heights() {
        let view = auto_view();
        let placeholder = body_height_for_row(&view, &[0], None);
        let measured = HashMap::from([(0, 999.0), (1, 300.0)]);
        let with_measured = body_height_for_row(&view, &[0, 1], Some(&measured));
        assert_eq!(with_measured, 999.0, "measured height must win");
        assert_ne!(
            placeholder, 999.0,
            "placeholder must not be the measured value"
        );
    }
}
