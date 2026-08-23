//! Card image resolution for the cards view.

use super::super::snapshot::BaseRow;
use crate::vault::VaultCatalog;

#[derive(Clone, Debug)]
pub(in crate::ui::viewers::base) enum CardImage {
    Local(std::path::PathBuf),
    External(String),
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
