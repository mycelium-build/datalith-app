use std::path::{Path, PathBuf};

use gpui_kit::{
    AnyElement, App, FocusHandle, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, img, px,
};

pub const IMAGE_MAX_WIDTH: f32 = 800.0;

pub struct ImageViewer {
    file_path: PathBuf,
    focus: FocusHandle,
}

impl ImageViewer {
    pub fn new(file_path: PathBuf, cx: &App) -> Self {
        Self {
            file_path,
            focus: cx.focus_handle(),
        }
    }

    #[must_use]
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    pub fn render(&self, _cx: &mut App) -> AnyElement {
        div()
            .id("image-viewer")
            .track_focus(&self.focus)
            .flex_1()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .items_center()
            .p_4()
            .child(
                img(image_source(&self.file_path))
                    .w_full()
                    .max_w(px(IMAGE_MAX_WIDTH)),
            )
            .into_any_element()
    }
}

/// Resolve a vault image without materializing embedded resources on disk.
#[must_use]
pub fn image_source(path: &Path) -> gpui_kit::ImageSource {
    if datalith_vault::source::is_read_only(path) {
        gpui_kit::ImageSource::Resource(gpui_kit::Resource::Embedded(
            path.to_string_lossy().into_owned().into(),
        ))
    } else {
        path.to_path_buf().into()
    }
}
