//! Kanban view configuration.

use anyhow::{Result, bail};

use crate::base::{CardImageFit, DisplayProperty, GroupRule, ViewKind};

/// The particular settings of a `type: kanban` view.
///
/// `group` is non-optional:
/// columns are the values of the grouped property,
/// so a Kanban view without a group cannot be constructed.
#[derive(Clone, Debug, PartialEq)]
pub struct KanbanConfig {
    pub group: GroupRule,
    pub image: Option<DisplayProperty>,
    pub image_fit: CardImageFit,
    pub image_aspect_ratio: f32,
    pub column_width: f32,
}

pub fn build(
    group: GroupRule,
    image: Option<String>,
    image_fit: Option<CardImageFit>,
    image_aspect_ratio: Option<f32>,
    column_width: Option<f32>,
) -> Result<ViewKind> {
    let image = image
        .map(|source| super::parse::display_property(&source))
        .transpose()?;
    let image_aspect_ratio = image_aspect_ratio.unwrap_or(1.0);
    if !image_aspect_ratio.is_finite() || image_aspect_ratio <= 0.0 {
        bail!("view.imageAspectRatio must be greater than 0");
    }
    let column_width = column_width.unwrap_or(260.0);
    if !column_width.is_finite() || column_width <= 0.0 {
        bail!("view.columnWidth must be greater than 0");
    }
    Ok(ViewKind::Kanban(KanbanConfig {
        group,
        image,
        image_fit: image_fit.unwrap_or_default(),
        image_aspect_ratio,
        column_width,
    }))
}
