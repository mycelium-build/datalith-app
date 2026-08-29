use gpui::{
    AnyElement, App, Context, ElementId, Entity, InteractiveElement, IntoElement, ObjectFit,
    ParentElement, Pixels, SharedUri, Size, Styled, Window, div, img, prelude::StyledImage as _,
    px, size,
};
use gpui_component::scroll::{ScrollableElement, Scrollbar, ScrollbarMode};
use gpui_component::{
    ActiveTheme, ElementExt, VirtualListScrollHandle, h_flex, v_flex, v_virtual_list,
};

use crate::document::base::BaseView;

use super::{BaseItem, BaseRow, BaseSnapshot, BaseStatus, BaseViewState};

const CARD_GAP: f32 = 16.0;
const CARD_BODY_MIN_HEIGHT: f32 = 96.0;
const CARD_BODY_PADDING: f32 = 24.0;
const CARD_PROPERTY_HEIGHT: f32 = 44.0;
const CARD_MIN_WIDTH: f32 = 120.0;
const GRID_PADDING: f32 = 16.0;

mod image;

pub(super) use image::{CardImage, collect_card_image_targets, resolve_card_image};

pub(super) struct CardsState {
    pub(super) scroll_handle: VirtualListScrollHandle,
    pub(super) viewport_width: Pixels,
    fullscreen_image: Option<CardImage>,
}

impl CardsState {
    pub(super) fn new() -> Self {
        Self {
            scroll_handle: VirtualListScrollHandle::new(),
            viewport_width: px(0.),
            fullscreen_image: None,
        }
    }

    fn show_fullscreen_image(&mut self, image: CardImage) {
        self.fullscreen_image = Some(image);
    }

    pub(super) fn render_fullscreen_image(&self, cx: &App) -> Option<AnyElement> {
        let image = self.fullscreen_image.as_ref()?;
        let content = match image {
            CardImage::Local(path) => img(path.clone())
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
}

pub(super) fn hide_fullscreen_image(
    state: &mut BaseViewState,
    _event: &gpui::MouseUpEvent,
    _window: &mut Window,
    _cx: &mut Context<BaseViewState>,
) {
    if let Some(cards) = state.cards.as_mut() {
        cards.fullscreen_image = None;
    }
}

struct CardRenderContext<'a> {
    snapshot: &'a BaseSnapshot,
    view: &'a BaseView,
    handler: &'a gpui::WeakEntity<crate::document::handler::FileHandler>,
    fullscreen_entity: &'a Entity<BaseViewState>,
    cx: &'a App,
}

impl BaseViewState {
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
            let item_sizes = card_row_sizes(snapshot, columns, card_width);
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
            .on_prepaint(move |bounds, _window, cx| {
                let width = content_width(bounds.size.width);
                viewport_entity.update(cx, |state, cx| {
                    if let Some(cards) = state.cards.as_mut()
                        && cards.viewport_width != width
                    {
                        cards.viewport_width = width;
                        cx.notify();
                    }
                });
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
    ((width + CARD_GAP) / (card_size + CARD_GAP))
        .floor()
        .to_string()
        .parse::<usize>()
        .unwrap_or(1)
        .max(1)
}

fn card_width_for(card_size: f32, viewport_width: Pixels, columns: usize) -> f32 {
    let width = f32::from(viewport_width);
    if width <= 0.0 {
        return card_size;
    }
    let gaps = CARD_GAP
        * columns
            .saturating_sub(1)
            .to_string()
            .parse::<f32>()
            .unwrap_or(0.0);
    ((width - gaps) / columns.to_string().parse::<f32>().unwrap_or(1.0)).max(CARD_MIN_WIDTH)
}

fn card_row_sizes(snapshot: &BaseSnapshot, columns: usize, card_width: f32) -> Vec<Size<Pixels>> {
    let Some(view) = snapshot.definition.views.get(snapshot.view_index) else {
        return Vec::new();
    };
    let image_height = view
        .as_cards()
        .and_then(|cards| Some((cards, cards.image.as_ref()?)))
        .map_or(0.0, |(cards, _)| card_width / cards.image_aspect_ratio);
    let row_height = image_height + card_body_height(view) + CARD_GAP;
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
            CardItem::GridRow(_) => size(px(1.0), px(row_height)),
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
        .gap(px(CARD_GAP))
        .children(cards)
        .into_any_element()
}

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
                context.cx,
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
    v_flex()
        .id(card_id)
        .w(px(card_width))
        .h(px(card_height(context.view, card_width)))
        .flex_shrink_0()
        .overflow_hidden()
        .rounded(px(6.0))
        .border_1()
        .border_color(context.cx.theme().border)
        .bg(context.cx.theme().secondary)
        .children(image)
        .child(
            div().flex_1().min_h_0().overflow_y_scrollbar().child(
                v_flex()
                    .whitespace_normal()
                    .gap_1()
                    .p_3()
                    .children(properties),
            ),
        )
        .into_any_element()
}

fn card_height(view: &BaseView, card_width: f32) -> f32 {
    let body = card_body_height(view);
    let image_height = view
        .as_cards()
        .and_then(|cards| Some((cards, cards.image.as_ref()?)))
        .map_or(0.0, |(cards, _)| card_width / cards.image_aspect_ratio);
    image_height + body
}

fn card_body_height(view: &BaseView) -> f32 {
    let cards_config = view.as_cards().cloned().unwrap_or_default();
    let image_source = cards_config
        .image
        .as_ref()
        .map(|property| property.source.as_str());
    let property_count = view
        .order
        .iter()
        .filter(|property| image_source != Some(property.source.as_str()))
        .count()
        .max(1);
    let property_count = property_count.to_string().parse::<f32>().unwrap_or(1.0);
    (property_count - 1.0)
        .mul_add(
            4.0,
            CARD_BODY_PADDING + property_count * CARD_PROPERTY_HEIGHT,
        )
        .max(CARD_BODY_MIN_HEIGHT)
}

#[cfg(test)]
mod tests {
    use super::{BaseItem, CardItem, flatten_card_items};

    fn header(label: &str) -> BaseItem {
        BaseItem::Header {
            label: label.to_string(),
            count: 1,
            ordinal: 0,
        }
    }

    #[test]
    fn global_summary_leads_the_card_items_when_present() {
        let items = [
            header("done"),
            BaseItem::Row {
                index: 0,
                ordinal: 0,
            },
        ];
        let flat = flatten_card_items(&items, true, 3);
        assert!(matches!(flat[0], CardItem::GlobalSummary));
        assert!(matches!(flat[1], CardItem::Header { .. }));
        assert!(matches!(flat[2], CardItem::GridRow(_)));

        let flat = flatten_card_items(&items, false, 3);
        assert!(
            !matches!(flat[0], CardItem::GlobalSummary),
            "no summaries, no header"
        );
    }
}
