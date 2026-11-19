//! Commands that a touch keyboard hides or buries (`[[` sits behind two
//! layouts on an AZERTY iPhone). The bar sits at the bottom of the visible
//! bounds, so it rides above the keyboard while a note is in Edit. Its
//! commands scroll sideways; hiding the keyboard stays pinned at the end.
use super::Workspace;
use datalith_ui::actions::{
    Bold, InsertTag, InsertWikiLink, Italic, Strikethrough, ToggleChecklist, ToggleHeading,
};
use datalith_ui::icons::DatalithIcon;
use datalith_ui::touch::TouchSize as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Redo, Undo};
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, h_flex};
use gpui_kit::prelude::*;
use gpui_kit::{Action, Context, FontWeight, Window, div, px};

fn key(id: &'static str, label: &'static str) -> Button {
    Button::new(id)
        .ghost()
        .touch_w(48.)
        .touch_h(44.)
        .accessibility_label(label)
}

/// How a key shows its command: a pixel icon, or a styled letter.
enum Glyph {
    Pixel(DatalithIcon),
    Undo,
    Bold,
    Italic,
}

/// Id, spoken label, glyph and the editor action of one key.
type Command = (&'static str, &'static str, Glyph, fn() -> Box<dyn Action>);

pub(super) fn render(cx: &Context<Workspace>) -> impl IntoElement + use<> {
    let icon = Glyph::Pixel;
    let commands: [Command; 9] = [
        ("undo", "Undo", Glyph::Undo, || Undo.boxed_clone()),
        ("redo", "Redo", icon(DatalithIcon::Redo), || {
            Redo.boxed_clone()
        }),
        (
            "insert-link",
            "Insert link",
            icon(DatalithIcon::Link),
            || InsertWikiLink.boxed_clone(),
        ),
        ("insert-tag", "Insert tag", icon(DatalithIcon::Hash), || {
            InsertTag.boxed_clone()
        }),
        ("heading", "Heading", icon(DatalithIcon::Heading), || {
            ToggleHeading.boxed_clone()
        }),
        (
            "checklist",
            "Checklist item",
            icon(DatalithIcon::Todo),
            || ToggleChecklist.boxed_clone(),
        ),
        ("bold", "Bold", Glyph::Bold, || Bold.boxed_clone()),
        ("italic", "Italic", Glyph::Italic, || Italic.boxed_clone()),
        (
            "strikethrough",
            "Strikethrough",
            icon(DatalithIcon::Strikethrough),
            || Strikethrough.boxed_clone(),
        ),
    ];
    let keys = commands.map(|(id, label, glyph, action)| {
        let button = key(id, label);
        let button = match glyph {
            Glyph::Pixel(icon) => button.icon(Icon::new(icon)),
            Glyph::Undo => button.icon(IconName::Undo),
            Glyph::Bold => button.child(div().font_weight(FontWeight::BOLD).child("B")),
            Glyph::Italic => button.child(div().italic().child("I")),
        };
        button.on_click(
            cx.listener(move |v: &mut Workspace, _, w, cx| v.editor_action(action(), w, cx)),
        )
    });
    h_flex()
        .min_h(px(44.))
        .border_t_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary)
        .child(
            h_flex()
                .id("keyboard-commands")
                .flex_1()
                .min_w_0()
                .px_1()
                .gap_1()
                .overflow_x_scroll()
                .children(keys),
        )
        .child(
            key("hide-keyboard", "Hide keyboard")
                .icon(IconName::ChevronDown)
                .on_click(cx.listener(|_, _, w: &mut Window, _| w.dismiss_virtual_keyboard())),
        )
}
