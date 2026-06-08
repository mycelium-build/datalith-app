use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

pub const PEN_ICON: &str = "icons/pen.svg";
const PEN_ICON_SVG: &str = include_str!("../assets/icons/pen.svg");

pub struct DatalithAssets;

impl AssetSource for DatalithAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match path {
            PEN_ICON => Ok(Some(Cow::Borrowed(PEN_ICON_SVG.as_bytes()))),
            _ => gpui_component_assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut entries = gpui_component_assets::Assets.list(path)?;
        if path.starts_with("icons/") && !entries.iter().any(|e| e == PEN_ICON) {
            entries.push(PEN_ICON.into());
        }
        Ok(entries)
    }
}
