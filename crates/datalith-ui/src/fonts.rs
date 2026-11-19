use gpui_kit::{Global, SharedString};
use std::borrow::Cow;

#[derive(Clone)]
pub struct ResolvedFonts {
    pub interface: SharedString,
    pub reading: SharedString,
    pub headings: SharedString,
    pub code: SharedString,
}

impl ResolvedFonts {
    #[must_use]
    pub const fn interface(&self) -> &SharedString {
        &self.interface
    }

    #[must_use]
    pub const fn reading(&self) -> &SharedString {
        &self.reading
    }

    #[must_use]
    pub const fn headings(&self) -> &SharedString {
        &self.headings
    }

    #[must_use]
    pub const fn code(&self) -> &SharedString {
        &self.code
    }
}

impl Global for ResolvedFonts {}

pub const PIXELOID_FONT: &str = "Pixeloid Sans";

/// Registers the shared Datalith heading font.
///
/// # Errors
/// Returns the text backend error if the font cannot be registered.
pub fn load_pixeloid(cx: &gpui_kit::App) -> gpui_kit::Result<()> {
    static REGULAR: &[u8] = include_bytes!("../../../assets/fonts/Pixeloid/PixeloidSans.ttf");
    static BOLD: &[u8] = include_bytes!("../../../assets/fonts/Pixeloid/PixeloidSans-Bold.ttf");
    let fonts = vec![Cow::Borrowed(REGULAR), Cow::Borrowed(BOLD)];
    cx.text_system().add_fonts(fonts)
}

pub(crate) fn editor_font_size(cx: &gpui_kit::App) -> f32 {
    use gpui_kit::component::ActiveTheme as _;
    if cfg!(any(target_os = "ios", target_os = "android")) {
        f32::from(cx.theme().font_size)
    } else {
        crate::BASE_FONT_SIZE
    }
}
