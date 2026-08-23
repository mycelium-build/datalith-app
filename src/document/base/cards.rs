//! Cards view configuration.

use anyhow::{Result, bail};

use crate::document::base::{CardImageFit, DisplayProperty, ViewKind};

/// The particular settings of a `type: cards` view.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CardsConfig {
    pub image: Option<DisplayProperty>,
    pub image_fit: CardImageFit,
    pub image_aspect_ratio: f32,
    pub card_size: f32,
}

pub fn build(
    image: Option<String>,
    image_fit: Option<CardImageFit>,
    image_aspect_ratio: Option<f32>,
    card_size: Option<f32>,
) -> Result<ViewKind> {
    let image = image
        .map(|source| super::parse::display_property(&source))
        .transpose()?;
    let image_aspect_ratio = image_aspect_ratio.unwrap_or(1.0);
    if !image_aspect_ratio.is_finite() || image_aspect_ratio <= 0.0 {
        bail!("view.imageAspectRatio must be greater than 0");
    }
    let card_size = card_size.unwrap_or(200.0);
    if !card_size.is_finite() || card_size <= 0.0 {
        bail!("view.cardSize must be greater than 0");
    }
    Ok(ViewKind::Cards(CardsConfig {
        image,
        image_fit: image_fit.unwrap_or_default(),
        image_aspect_ratio,
        card_size,
    }))
}
