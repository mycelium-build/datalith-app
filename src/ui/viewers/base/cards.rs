use gpui::{
    AnyElement, App, Context, ElementId, Entity, InteractiveElement, IntoElement, MouseButton,
    MouseUpEvent, ObjectFit, ParentElement, Pixels, SharedUri, Size, Styled, Window, div, img,
    prelude::StyledImage, px, size,
};
use gpui_component::scroll::{ScrollableElement, Scrollbar, ScrollbarMode};
use gpui_component::{
    ActiveTheme, ElementExt, VirtualListScrollHandle, h_flex, v_flex, v_virtual_list,
};

use crate::document::base::{BaseView, CardImageFit};
use crate::document::filter::PropertyPath;
use crate::vault::VaultCatalog;

use super::{BaseRow, BaseSnapshot, BaseStatus, BaseViewState};

const CARD_GAP: f32 = 16.0;
const CARD_BODY_MIN_HEIGHT: f32 = 96.0;
const CARD_BODY_PADDING: f32 = 24.0;
const CARD_PROPERTY_HEIGHT: f32 = 44.0;
const CARD_MIN_WIDTH: f32 = 120.0;
const GRID_PADDING: f32 = 16.0;

#[derive(Clone, Debug)]
pub(super) enum CardImage {
    Local(std::path::PathBuf),
    External(String),
}

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
}

pub(super) fn hide_fullscreen_image(
    state: &mut BaseViewState,
    _event: &MouseUpEvent,
    _window: &mut Window,
    cx: &mut Context<BaseViewState>,
) {
    if state
        .cards
        .as_mut()
        .and_then(|cards| cards.fullscreen_image.take())
        .is_some()
    {
        cx.notify();
    }
}

impl CardsState {
    pub(super) fn render_fullscreen_image(
        &self,
        cx: &Context<BaseViewState>,
    ) -> Option<AnyElement> {
        self.fullscreen_image.as_ref().map(|image| {
            let image = match image {
                CardImage::Local(path) => img(path.clone())
                    .size_full()
                    .object_fit(ObjectFit::Contain)
                    .into_any_element(),
                CardImage::External(url) => img(SharedUri::from(url.clone()))
                    .size_full()
                    .object_fit(ObjectFit::Contain)
                    .into_any_element(),
            };
            div()
                .id("base-image-fullscreen")
                .absolute()
                .inset_0()
                .bg(cx.theme().background)
                .p_4()
                .flex()
                .items_center()
                .justify_center()
                .child(image)
                .into_any_element()
        })
    }
}

pub(super) fn resolve_card_image(
    property: &PropertyPath,
    row: &BaseRow,
    catalog: &VaultCatalog,
    root: &std::path::Path,
) -> Option<CardImage> {
    let value = super::property_value(property, row)?.as_str()?;
    let target = normalize_card_image_target(value);
    if target.is_empty() {
        return None;
    }
    if target.starts_with("http://") || target.starts_with("https://") {
        return Some(CardImage::External(target));
    }
    let target = percent_encoding::percent_decode_str(&target)
        .decode_utf8_lossy()
        .to_string();
    let relative_candidate = row.path.parent().map_or_else(
        || root.join(&target),
        |parent| root.join(parent).join(&target),
    );
    if relative_candidate.is_file() {
        return Some(CardImage::Local(relative_candidate));
    }
    catalog
        .resolve(&target)
        .filter(|path| path.is_file())
        .map(CardImage::Local)
}

fn normalize_card_image_target(value: &str) -> String {
    let value = value.trim();
    let value = value
        .strip_prefix("![[")
        .and_then(|value| value.strip_suffix("]]"))
        .or_else(|| {
            value
                .strip_prefix("[[")
                .and_then(|value| value.strip_suffix("]]"))
        })
        .unwrap_or(value);
    let value = value
        .split_once("](")
        .map_or(value, |(_, value)| value.strip_suffix(')').unwrap_or(value));
    value
        .split_once('|')
        .map_or(value, |(target, _)| target)
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .to_string()
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
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(cards_state) = self.cards.as_ref() else {
            return super::centered_message("Cards view state is missing", cx);
        };
        let entity = cx.entity();
        let viewport_width = if cards_state.viewport_width == px(0.) {
            content_width(window.bounds().size.width)
        } else {
            cards_state.viewport_width
        };
        let columns = columns_for(view.card_size, viewport_width);
        let card_width = card_width_for(view.card_size, viewport_width, columns);
        let item_sizes = card_row_sizes(snapshot, view, columns, card_width);
        let handler = self.handler.clone();
        let fullscreen_entity = entity.clone();
        let list_entity = entity.clone();
        let list = v_virtual_list(
            list_entity,
            "base-cards",
            item_sizes.into(),
            move |state, visible_range, _, cx| {
                let (BaseStatus::Ready(snapshot) | BaseStatus::Empty(snapshot)) = &state.status
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
                visible_range
                    .map(|row_index| render_card_row(row_index, columns, card_width, &context))
                    .collect()
            },
        )
        .track_scroll(&cards_state.scroll_handle)
        .size_full();
        let viewport_entity = entity;
        div()
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
            })
            .child(div().size_full().p_4().child(list))
            .child(
                div().absolute().inset_0().child(
                    Scrollbar::vertical(&cards_state.scroll_handle)
                        .mode(ScrollbarMode::Always)
                        .viewport_from_layout(),
                ),
            )
            .into_any_element()
    }
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

fn card_row_sizes(
    snapshot: &BaseSnapshot,
    view: &BaseView,
    columns: usize,
    card_width: f32,
) -> Vec<Size<Pixels>> {
    let row_count = snapshot.rows.len().div_ceil(columns);
    let image_height = view
        .image
        .as_ref()
        .map_or(0.0, |_| card_width / view.image_aspect_ratio);
    let row_height = image_height + card_body_height(view) + CARD_GAP;
    vec![size(px(1.0), px(row_height)); row_count]
}

fn render_card_row(
    row_index: usize,
    columns: usize,
    card_width: f32,
    context: &CardRenderContext<'_>,
) -> AnyElement {
    let first = row_index.saturating_mul(columns);
    let end = first
        .saturating_add(columns)
        .min(context.snapshot.rows.len());
    let cards = (first..end)
        .map(|index| render_card(index, card_width, context.snapshot.rows.get(index), context));
    h_flex()
        .w_full()
        .gap(px(CARD_GAP))
        .children(cards)
        .into_any_element()
}

fn render_card(
    index: usize,
    card_width: f32,
    row: Option<&BaseRow>,
    context: &CardRenderContext<'_>,
) -> AnyElement {
    let Some(row) = row else {
        return div().w(px(card_width)).into_any_element();
    };
    let card_id =
        ElementId::NamedInteger("base-card".into(), u64::try_from(index).unwrap_or_default());
    let image = row.image.clone().map(|image| {
        render_card_image(
            image,
            card_width,
            context.view,
            context.fullscreen_entity.clone(),
            context.cx,
        )
    });
    let image_source = context
        .view
        .image
        .as_ref()
        .map(|property| property.source.as_str());
    let properties = context
        .view
        .order
        .iter()
        .enumerate()
        .filter(|(_, property)| image_source != Some(property.source.as_str()))
        .map(|(column, property)| {
            let cell = super::render_property_cell(
                row,
                property,
                context.handler,
                index,
                column,
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
                                    .display_name(property)
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
    let image_height = view
        .image
        .as_ref()
        .map_or(0.0, |_| card_width / view.image_aspect_ratio);
    image_height + card_body_height(view)
}

fn card_body_height(view: &BaseView) -> f32 {
    let image_source = view.image.as_ref().map(|property| property.source.as_str());
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

fn render_card_image(
    image: CardImage,
    card_width: f32,
    view: &BaseView,
    fullscreen_entity: Entity<BaseViewState>,
    cx: &App,
) -> AnyElement {
    let image_height = card_width / view.image_aspect_ratio;
    let object_fit = match view.image_fit {
        CardImageFit::Cover => ObjectFit::Cover,
        CardImageFit::Contain => ObjectFit::Contain,
    };
    let mut container = div()
        .w_full()
        .h(px(image_height))
        .overflow_hidden()
        .bg(cx.theme().background);
    let preview_image = image.clone();
    container = container
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            fullscreen_entity.update(cx, |state, cx| {
                if let Some(cards) = state.cards.as_mut() {
                    cards.show_fullscreen_image(preview_image.clone());
                    cx.notify();
                }
            });
        });
    let image_element = match image {
        CardImage::Local(path) => img(path)
            .size_full()
            .object_fit(object_fit)
            .into_any_element(),
        CardImage::External(url) => img(SharedUri::from(url))
            .size_full()
            .object_fit(object_fit)
            .into_any_element(),
    };
    container.child(image_element).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::normalize_card_image_target;

    #[test]
    fn normalizes_card_image_targets() {
        assert_eq!(
            normalize_card_image_target("![](https://example.com/image.png)"),
            "https://example.com/image.png"
        );
        assert_eq!(
            normalize_card_image_target("![[folder/image.png|Preview]]"),
            "folder/image.png"
        );
    }
}
