//! Inline choice for touch layouts. A `DropdownMenu` row is 1.625 rem
//! (about 26 px), under the 44 pt / 48 dp touch minimum, so touch views open
//! their options in place, as 48 px rows, under a trigger that says what is
//! chosen. Back closes an open choice before leaving the Base.
use gpui_kit::component::{
    IconName,
    button::{Button, ButtonVariants as _},
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    ElementId, InteractiveElement as _, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, px,
};

pub(super) const ROW_HEIGHT: f32 = 48.;

/// The trigger: the current value, with a chevron for the open state.
pub(super) fn trigger(
    id: impl Into<ElementId>,
    value: impl Into<SharedString>,
    purpose: &str,
    open: bool,
) -> Button {
    let value = value.into();
    Button::new(id)
        .ghost()
        .h(px(ROW_HEIGHT))
        .icon(if open {
            IconName::ChevronUp
        } else {
            IconName::ChevronDown
        })
        .accessibility_label(format!("{purpose}, current {value}"))
        .label(value)
}

/// One option; the chosen one carries a check.
pub(super) fn option(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    chosen: bool,
) -> Button {
    let label = label.into();
    Button::new(id)
        .ghost()
        .w_full()
        .h(px(ROW_HEIGHT))
        .flex_shrink_0()
        .accessibility_label(label.clone())
        .label(label)
        .when(chosen, |b| b.icon(IconName::Check))
}

/// The scrollable options, capped so the view stays visible behind them.
pub(super) fn options(
    id: impl Into<ElementId>,
    rows: impl IntoIterator<Item = Button>,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    v_flex()
        .id(id)
        .overflow_y_scroll()
        .max_h(px(ROW_HEIGHT * 5.5))
        .children(rows)
}
