//! Card image resolution and rendering for the cards view.

use gpui::{
    AnyElement, App, Entity, InteractiveElement, IntoElement, MouseButton, ObjectFit,
    ParentElement, SharedUri, Styled, div, img, prelude::StyledImage as _, px,
};
use gpui_component::ActiveTheme;

use crate::document::base::{BaseView, CardImageFit};

use super::super::BaseViewState;
use super::super::snapshot::BaseRow;
use crate::vault::VaultCatalog;

#[derive(Clone, Debug)]
pub(in crate::ui::viewers::base) enum CardImage {
    Local(std::path::PathBuf),
    External(String),
}

pub(super) fn render_card_image(
    image: CardImage,
    card_width: f32,
    view: &BaseView,
    fullscreen_entity: Entity<BaseViewState>,
    cx: &App,
) -> AnyElement {
    let cards_config = view.as_cards().cloned().unwrap_or_default();
    let image_height = card_width / cards_config.image_aspect_ratio;
    let object_fit = match cards_config.image_fit {
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

pub(in crate::ui::viewers::base) fn resolve_card_image(
    source: &str,
    row: &BaseRow,
    projection_index: &std::collections::HashMap<String, usize>,
    catalog: &VaultCatalog,
    root: &std::path::Path,
) -> Option<CardImage> {
    let position = projection_index.get(source)?;
    let value = row.values.get(*position)?.as_str()?;
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

pub(in crate::ui::viewers::base) fn normalize_card_image_target(value: &str) -> String {
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
