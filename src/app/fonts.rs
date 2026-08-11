use std::borrow::Cow;

use gpui::App;

pub const PIXELOID_FONT: &str = "Pixeloid Sans";

pub fn load_embedded_fonts(cx: &App) {
    static REGULAR: &[u8] = include_bytes!("../../assets/fonts/PixeloidSans.ttf");
    static BOLD: &[u8] = include_bytes!("../../assets/fonts/PixeloidSans-Bold.ttf");
    cx.text_system()
        .add_fonts(vec![Cow::Borrowed(REGULAR), Cow::Borrowed(BOLD)])
        .ok();
}
