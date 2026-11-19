//! Commands that a touch keyboard hides or buries (`[[` sits behind two
//! layouts on an AZERTY iPhone). The bar sits at the bottom of the visible
//! bounds, so it rides above the keyboard while a note is in Edit.
use super::Workspace;
use datalith_ui::actions::{Bold, InsertWikiLink, Italic};
use datalith_ui::icons::DatalithIcon;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, h_flex};
use gpui_kit::prelude::*;
use gpui_kit::{Action, Context, FontWeight, Window, div, px};

fn key(id: &'static str, label: &'static str) -> Button {
    Button::new(id)
        .ghost()
        .w(px(48.))
        .h(px(44.))
        .accessibility_label(label)
}

pub(super) fn render(cx: &Context<Workspace>) -> impl IntoElement + use<> {
    let send = |action: fn() -> Box<dyn Action>| {
        cx.listener(move |v: &mut Workspace, _, w, cx| v.editor_action(action(), w, cx))
    };
    h_flex()
        .px_1()
        .gap_1()
        .min_h(px(44.))
        .border_t_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary)
        .child(
            key("insert-link", "Insert link")
                .icon(Icon::new(DatalithIcon::Link))
                .on_click(send(|| InsertWikiLink.boxed_clone())),
        )
        .child(
            key("bold", "Bold")
                .child(div().font_weight(FontWeight::BOLD).child("B"))
                .on_click(send(|| Bold.boxed_clone())),
        )
        .child(
            key("italic", "Italic")
                .child(div().italic().child("I"))
                .on_click(send(|| Italic.boxed_clone())),
        )
        .child(
            key("undo", "Undo")
                .icon(IconName::Undo)
                .on_click(send(|| gpui_kit::component::input::Undo.boxed_clone())),
        )
        .child(div().flex_1())
        .child(
            key("hide-keyboard", "Hide keyboard")
                .icon(IconName::ChevronDown)
                .on_click(cx.listener(|_, _, w: &mut Window, _| w.dismiss_virtual_keyboard())),
        )
}
