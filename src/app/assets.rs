use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

pub const PEN_ICON: &str = "icons/pen.svg";
const PEN_ICON_SVG: &str = include_str!("../../assets/icons/pen.svg");

pub const ARROW_DOWN_AZ_ICON: &str = "icons/arrow-down-a-z.svg";
const ARROW_DOWN_AZ_SVG: &str = include_str!("../../assets/icons/arrow-down-a-z.svg");

pub const ARROW_UP_AZ_ICON: &str = "icons/arrow-up-a-z.svg";
const ARROW_UP_AZ_SVG: &str = include_str!("../../assets/icons/arrow-up-a-z.svg");

pub const FUNNEL_ICON: &str = "icons/funnel.svg";
const FUNNEL_ICON_SVG: &str = include_str!("../../assets/icons/funnel.svg");

pub struct DatalithAssets;

impl AssetSource for DatalithAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match path {
            PEN_ICON => Ok(Some(Cow::Borrowed(PEN_ICON_SVG.as_bytes()))),
            ARROW_DOWN_AZ_ICON => Ok(Some(Cow::Borrowed(ARROW_DOWN_AZ_SVG.as_bytes()))),
            ARROW_UP_AZ_ICON => Ok(Some(Cow::Borrowed(ARROW_UP_AZ_SVG.as_bytes()))),
            FUNNEL_ICON => Ok(Some(Cow::Borrowed(FUNNEL_ICON_SVG.as_bytes()))),
            _ => gpui_component_assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut entries = gpui_component_assets::Assets.list(path)?;
        if path.starts_with("icons/") {
            for icon in [PEN_ICON, ARROW_DOWN_AZ_ICON, ARROW_UP_AZ_ICON, FUNNEL_ICON] {
                if !entries.iter().any(|e| e == icon) {
                    entries.push(icon.into());
                }
            }
        }
        Ok(entries)
    }
}
