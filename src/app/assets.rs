use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

use crate::ui::icons::ICON_ASSETS;

pub struct DatalithAssets;

impl AssetSource for DatalithAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some((_, bytes)) = ICON_ASSETS.iter().find(|(icon, _)| *icon == path) {
            return Ok(Some(Cow::Borrowed(bytes.as_bytes())));
        }
        gpui_component_assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut entries = gpui_component_assets::Assets.list(path)?;
        if path.starts_with("icons/") {
            for (icon, _) in ICON_ASSETS {
                if !entries.iter().any(|entry| entry == icon) {
                    entries.push((*icon).into());
                }
            }
        }
        Ok(entries)
    }
}
