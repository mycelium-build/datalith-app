use gpui_kit::StyledImage as _;
use std::path::{Path, PathBuf};

use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement,
    IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled, Window, div, img, px,
};

pub const IMAGE_MAX_WIDTH: f32 = 800.0;

pub struct ImageViewer {
    file_path: PathBuf,
    focus: FocusHandle,
    touch: Option<Entity<TouchImage>>,
}

impl ImageViewer {
    pub fn new(file_path: PathBuf, cx: &mut App) -> Self {
        let touch = cfg!(any(target_os = "ios", target_os = "android")).then(|| {
            cx.new(|_| TouchImage {
                path: file_path.clone(),
                scale: 1.,
                scroll: gpui_kit::ScrollHandle::new(),
            })
        });
        Self {
            touch,
            file_path,
            focus: cx.focus_handle(),
        }
    }

    #[must_use]
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    pub fn render(&self, _cx: &mut App) -> AnyElement {
        if let Some(touch) = &self.touch {
            return div()
                .size_full()
                .track_focus(&self.focus)
                .child(touch.clone())
                .into_any_element();
        }
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
    if datalith_vault::source::is_read_only(path) || datalith_vault::source::is_provider(path) {
        gpui_kit::ImageSource::Resource(gpui_kit::Resource::Embedded(
            path.to_string_lossy().into_owned().into(),
        ))
    } else {
        path.to_path_buf().into()
    }
}

struct TouchImage {
    path: PathBuf,
    scale: f32,
    scroll: gpui_kit::ScrollHandle,
}

impl Render for TouchImage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use gpui_kit::component::{
            button::{Button, ButtonVariants as _},
            h_flex, v_flex,
        };
        let controls = [
            ("zoom-out", "−", "Zoom out", 0.8),
            ("zoom-in", "+", "Zoom in", 1.25),
            ("fit", "⊡", "Fit image", 0.),
        ]
        .map(|(id, symbol, label, factor)| {
            Button::new(id)
                .ghost()
                .w(px(48.))
                .h(px(48.))
                .label(symbol)
                .accessibility_label(label)
                .on_click(cx.listener(move |s, _, _, cx| {
                    if factor == 0. {
                        s.scale = 1.;
                        s.scroll.set_offset(gpui_kit::Point::default());
                    } else {
                        s.scale = (s.scale * factor).clamp(1., 8.);
                    }
                    cx.notify();
                }))
        });
        v_flex()
            .size_full()
            .child(
                div()
                    .id("image-pan")
                    .min_h_0()
                    .flex_1()
                    .overflow_scroll()
                    .track_scroll(&self.scroll)
                    .on_pinch(cx.listener(|s, event: &gpui_kit::PinchEvent, _, cx| {
                        s.scale = (s.scale * (1. + event.delta)).clamp(1., 8.);
                        cx.notify();
                    }))
                    .child(
                        img(image_source(&self.path))
                            .w(gpui_kit::relative(self.scale))
                            .h(gpui_kit::relative(self.scale))
                            .object_fit(gpui_kit::ObjectFit::Contain),
                    ),
            )
            .child(h_flex().justify_center().children(controls))
    }
}
