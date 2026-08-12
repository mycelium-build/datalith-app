use std::borrow::Cow;

use gpui::App;
use gpui_component::notification::Notification;

use crate::ui::notifications;

pub const PIXELOID_FONT: &str = "Pixeloid Sans";

pub fn load_embedded_fonts(cx: &App) -> Vec<Notification> {
    static REGULAR: &[u8] = include_bytes!("../../assets/fonts/Pixeloid/PixeloidSans.ttf");
    static BOLD: &[u8] = include_bytes!("../../assets/fonts/Pixeloid/PixeloidSans-Bold.ttf");
    let fonts = vec![Cow::Borrowed(REGULAR), Cow::Borrowed(BOLD)];
    match cx.text_system().add_fonts(fonts) {
        Ok(()) => Vec::new(),
        Err(error) => vec![notifications::font_load_failed(&error)],
    }
}
