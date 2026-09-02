//! List view configuration.

use anyhow::{Result, bail};

use crate::document::base::{ListMarkers, ViewKind};

/// The particular settings of a `type: list` view.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListConfig {
    pub markers: ListMarkers,
    pub indent_properties: bool,
    pub separators: String,
}

pub fn build(
    markers: Option<ListMarkers>,
    indent_properties: Option<bool>,
    separators: Option<String>,
    view_name: &str,
) -> Result<ViewKind> {
    let separators = separators.unwrap_or_else(|| ", ".into());
    if separators.is_empty() {
        bail!("view {view_name:?}.separators must not be empty");
    }
    Ok(ViewKind::List(ListConfig {
        markers: markers.unwrap_or_default(),
        indent_properties: indent_properties.unwrap_or(false),
        separators,
    }))
}
