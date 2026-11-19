//! GPUI top bar for hosts without a native navigation bar (Android). It
//! renders the same `ScreenControls` that `UIKit` receives on iOS:
//! `[Back] Title … [Edit|Done] [+] [⋮]`. Overflow and creation open the
//! platform's action sheet so Delete never sits next to Edit.
use super::{Choice, Command, ScreenControls, Workspace};
use datalith_ui::icons::DatalithIcon;
use datalith_ui::touch::TouchSize as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, h_flex};
use gpui_kit::prelude::*;
use gpui_kit::{Context, div};

/// Material's top app bar: 48 dp targets holding 24 dp icons.
fn icon_button(id: &'static str, label: &str) -> Button {
    Button::new(id)
        .ghost()
        .large()
        .touch_size(48.)
        .accessibility_label(label.to_owned())
}

pub(super) fn render(controls: &ScreenControls, cx: &Context<Workspace>) -> impl IntoElement {
    let title = controls.title.clone().unwrap_or_default();
    let mut bar = h_flex()
        .gap_1()
        .px_1()
        .touch_h(56.)
        .border_b_1()
        .border_color(cx.theme().border)
        .when(controls.back, |bar| {
            bar.child(
                icon_button("back", Command::Back.label())
                    .icon(IconName::ChevronLeft)
                    .on_click(cx.listener(|v, _, w, cx| v.run(Command::Back, None, w, cx))),
            )
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .px_2()
                .text_lg()
                .truncate()
                .child(title),
        );
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
    let create = controls.create_commands();
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
    let menu = controls.menu_commands();
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
    bar
}
