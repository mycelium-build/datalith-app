use std::path::PathBuf;

use gpui::{
    AnyElement, App, FocusHandle, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, img, px,
};

pub const IMAGE_MAX_WIDTH: f32 = 800.0;

pub struct ImageViewer {
    file_path: PathBuf,
}

impl ImageViewer {
    pub const fn new(file_path: PathBuf) -> Self {
        Self { file_path }
    }

    // Kept as a method so every viewer exposes the same focus_handle(&self, cx) signature.
    #[allow(clippy::unused_self)]
    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        cx.focus_handle()
    }

    pub fn render(&self, _cx: &mut App) -> AnyElement {
        div()
            .id("image-viewer")
            .flex_1()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .items_center()
            .p_4()
            .child(
                img(self.file_path.clone())
                    .w_full()
                    .max_w(px(IMAGE_MAX_WIDTH)),
            )
            .into_any_element()
    }
}
