//! GPUI bars for hosts without native bars (Android). They render the same
//! `ScreenControls` that `UIKit` receives on iOS, with the same placement:
//! a light top bar for the document title and its primary action (Edit or
//! Done), and a bottom bar within thumb reach for Back, New and More.
//! Overflow and creation open the platform's action sheet, so Delete never
//! sits next to Edit.
use super::{Choice, Command, ScreenControls, Workspace};
use datalith_ui::icons::DatalithIcon;
use datalith_ui::touch::TouchSize as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, h_flex};
use gpui_kit::prelude::*;
use gpui_kit::{Context, div};

/// Material's 48 dp targets holding 24 dp icons.
fn icon_button(id: &'static str, label: &str) -> Button {
    Button::new(id)
        .ghost()
        .large()
        .touch_size(48.)
        .accessibility_label(label.to_owned())
}

/// The document title and its primary action, or nothing on screens that
/// have neither: a folder names itself in its own header.
pub(super) fn top(controls: &ScreenControls, cx: &Context<Workspace>) -> Option<impl IntoElement> {
    if controls.title.is_none() && controls.primary.is_none() {
        return None;
    }
    let title = controls.title.clone().unwrap_or_default();
    let mut bar = h_flex()
        .gap_1()
        .pl_3()
        .pr_1()
        .touch_h(48.)
        .child(div().flex_1().min_w_0().truncate().child(title));
    if let Some(primary) = &controls.primary {
        let editing = controls.editing;
        bar = bar.child(
            icon_button("primary", primary.label)
                .icon(if editing {
                    Icon::new(IconName::Check)
                } else {
                    Icon::new(DatalithIcon::Pen)
                })
                .on_click(cx.listener(|v, _, w, cx| v.run(Command::ToggleMode, None, w, cx))),
        );
    }
    Some(bar)
}

/// Back, New and More within thumb reach. While typing it steps aside: the
/// keyboard would cover it, and the keyboard bar takes its place in a note.
pub(super) fn bottom(
    controls: &ScreenControls,
    cx: &Context<Workspace>,
) -> Option<impl IntoElement> {
    let create = controls.create_commands();
    let menu = controls.menu_commands();
    if controls.typing || (!controls.back && create.is_empty() && menu.is_empty()) {
        return None;
    }
    let mut bar = h_flex()
        .gap_1()
        .px_2()
        .touch_h(56.)
        .border_t_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background);
    if controls.back {
        bar = bar.child(
            icon_button("back", Command::Back.label())
                .icon(IconName::ChevronLeft)
                .on_click(cx.listener(|v, _, w, cx| v.run(Command::Back, None, w, cx))),
        );
    }
    bar = bar.child(div().flex_1());
    if !create.is_empty() {
        bar = bar.child(
            icon_button("create", "New")
                .icon(IconName::Plus)
                .on_click(cx.listener(move |v, _, w, cx| {
                    let choice = Choice::Actions {
                        title: "New".into(),
                        target: None,
                        commands: create.clone(),
                    };
                    v.choose(choice, w, cx);
                })),
        );
    }
    if !menu.is_empty() {
        bar = bar.child(
            icon_button("more", "More actions")
                .icon(IconName::EllipsisVertical)
                .on_click(cx.listener(move |v, _, w, cx| {
                    let choice = Choice::Actions {
                        title: v.place_title(),
                        target: None,
                        commands: menu.clone(),
                    };
                    v.choose(choice, w, cx);
                })),
        );
    }
    Some(bar)
}
