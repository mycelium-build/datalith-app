//! Shortcut reference and remapping controls in a dedicated workspace tab.

use gpui_kit::component::{
    ActiveTheme, ChildElement, Disableable, Icon, IconName, Sizable, Size,
    button::{Button, ButtonVariants as _},
    h_flex,
    scroll::Scrollbar,
    table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow},
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, Context, FocusHandle, Focusable, FontWeight, InteractiveElement as _, IntoElement,
    KeyDownEvent, Keystroke, KeystrokeEvent, ParentElement, Render, RenderOnce, ScrollHandle,
    SharedString, StatefulInteractiveElement as _, Styled, Subscription, Window, div, point, px,
    rems,
};

use crate::app::keymap::{self, Shortcut, ShortcutRegistry};

struct ShortcutGroup {
    category: SharedString,
    rows: Vec<Shortcut>,
}

/// Adds a hover group while retaining the table's row semantics, index and sizing.
#[derive(IntoElement)]
struct ShortcutRow {
    id: SharedString,
    row: TableRow,
}

impl Sizable for ShortcutRow {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.row = self.row.with_size(size);
        self
    }
}

impl ChildElement for ShortcutRow {
    fn with_ix(mut self, ix: usize) -> Self {
        self.row = self.row.with_ix(ix);
        self
    }
}

impl RenderOnce for ShortcutRow {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .id(self.id.clone())
            .group(self.id)
            .w_full()
            .child(self.row)
    }
}

struct ShortcutCapture {
    id: &'static str,
    phase: CapturePhase,
}

enum CapturePhase {
    Listening,
    Proposed(String),
    Invalid(String),
}

pub struct ShortcutsView {
    focus: FocusHandle,
    scroll: ScrollHandle,
    registry: ShortcutRegistry,
    groups: Vec<ShortcutGroup>,
    capture: Option<ShortcutCapture>,
    settings_error: Option<String>,
    _capture_subscription: Subscription,
}

impl ShortcutsView {
    pub(crate) fn new(cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        let registry = ShortcutRegistry::new();
        let groups = shortcut_groups(registry.shortcuts());
        let weak_view = cx.weak_entity();
        let capture_subscription = cx.intercept_keystrokes(move |event, window, cx| {
            if let Some(view) = weak_view.upgrade() {
                view.update(cx, |view, cx| view.intercept_keystroke(event, window, cx));
            }
        });

        Self {
            focus,
            scroll: ScrollHandle::new(),
            registry,
            groups,
            capture: None,
            settings_error: None,
            _capture_subscription: capture_subscription,
        }
    }

    #[allow(
        clippy::arithmetic_side_effects,
        reason = "floating-point pixel arithmetic; scroll offsets are clamped to bounds"
    )]
    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.modifiers != gpui_kit::Modifiers::none() {
            return;
        }

        let current = self.scroll.offset();
        let max = self.scroll.max_offset().y;
        let line = window.rem_size() * 2.0;
        let page = self.scroll.bounds().size.height * 0.85;
        let next = match event.keystroke.key.as_str() {
            "down" => current.y - line,
            "up" => current.y + line,
            "pagedown" => current.y - page,
            "pageup" => current.y + page,
            "home" => px(0.),
            "end" => -max,
            _ => return,
        };
        self.scroll
            .set_offset(point(current.x, next.clamp(-max, px(0.))));
        cx.stop_propagation();
        cx.notify();
    }

    fn intercept_keystroke(
        &mut self,
        event: &KeystrokeEvent,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if self.capture.is_none() || !self.focus.contains_focused(window, cx) {
            return;
        }

        let key = event.keystroke.key.as_str();
        if is_modifier_key(key) {
            return;
        }
        if key == "tab" {
            if self
                .capture
                .as_ref()
                .is_some_and(|capture| !matches!(capture.phase, CapturePhase::Proposed(_)))
            {
                self.capture = None;
                cx.notify();
            }
            return;
        }
        if key == "escape" {
            self.capture = None;
            cx.stop_propagation();
            cx.notify();
            return;
        }

        let Some(capture) = self.capture.as_mut() else {
            return;
        };
        match capture.phase {
            CapturePhase::Listening | CapturePhase::Invalid(_) => {
                capture.phase = capture_binding(&event.keystroke).map_or_else(
                    || CapturePhase::Invalid("That key cannot be used. Try another.".into()),
                    CapturePhase::Proposed,
                );
            }
            CapturePhase::Proposed(_)
                if key == "enter" && event.keystroke.modifiers == gpui_kit::Modifiers::none() =>
            {
                return;
            }
            CapturePhase::Proposed(_) => {}
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn start_capture(&mut self, id: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        self.capture = Some(ShortcutCapture {
            id,
            phase: CapturePhase::Listening,
        });
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn save_capture(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ShortcutCapture {
            id,
            phase: CapturePhase::Proposed(binding),
        }) = self.capture.take()
        else {
            return;
        };
        self.apply_change(|registry, cx| registry.set(id, &binding, cx), window, cx);
    }

    fn remove_binding(&mut self, id: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_change(|registry, cx| registry.remove(id, cx), window, cx);
    }

    fn reset_binding(&mut self, id: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_change(|registry, cx| registry.reset(id, cx), window, cx);
    }

    fn reset_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_change(ShortcutRegistry::reset_all, window, cx);
    }

    fn apply_change(
        &mut self,
        change: impl FnOnce(&mut ShortcutRegistry, &mut App) -> anyhow::Result<()>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_error = change(&mut self.registry, cx).err().map(|error| {
            format!("Couldn’t save shortcuts. Changes apply for this session. {error}")
        });
        self.groups = shortcut_groups(self.registry.shortcuts());
        self.capture = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    #[allow(clippy::too_many_lines)]
    fn render_tables(&self, cx: &Context<Self>) -> impl IntoElement {
        let mut groups = v_flex().gap_5();

        for group in &self.groups {
            let mut body = TableBody::new();
            for shortcut in &group.rows {
                let id = shortcut.id();
                let row_id: SharedString = format!("shortcut-row-{id}").into();
                let current_binding = shortcut
                    .binding()
                    .map_or_else(|| "No shortcut".to_owned(), keymap::display_binding);
                let capture = self
                    .capture
                    .as_ref()
                    .filter(|capture| capture.id == shortcut.id());

                let mut controls = h_flex().items_center().gap_2();
                if let Some(capture) = capture {
                    match &capture.phase {
                        CapturePhase::Listening => {
                            controls = controls.child(
                                div()
                                    .id(format!("shortcut-capture-status-{}", shortcut.id()))
                                    .aria_label("Press a shortcut")
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Press a shortcut"),
                            );
                        }
                        CapturePhase::Proposed(binding) => {
                            let id = capture.id;
                            controls = controls
                                .child(
                                    div()
                                        .id(format!("shortcut-proposed-{}", shortcut.id()))
                                        .aria_label(keymap::display_binding(binding))
                                        .font_family(cx.theme().mono_font_family.clone())
                                        .child(keymap::display_binding(binding)),
                                )
                                .child(
                                    Button::new(format!("save-shortcut-{id}"))
                                        .small()
                                        .label("Save")
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.save_capture(window, cx);
                                        })),
                                )
                                .child(
                                    Button::new(format!("cancel-shortcut-{id}"))
                                        .ghost()
                                        .small()
                                        .label("Cancel")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.capture = None;
                                            this.focus.focus(window, cx);
                                            cx.notify();
                                        })),
                                );
                        }
                        CapturePhase::Invalid(message) => {
                            let id = capture.id;
                            controls = controls
                                .child(
                                    div()
                                        .id(format!("shortcut-capture-error-{}", shortcut.id()))
                                        .aria_label(message.clone())
                                        .text_color(cx.theme().danger)
                                        .child(message.clone()),
                                )
                                .child(
                                    Button::new(format!("capture-again-{id}"))
                                        .ghost()
                                        .small()
                                        .label("Try again")
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.start_capture(id, window, cx);
                                        })),
                                )
                                .child(
                                    Button::new(format!("cancel-shortcut-{id}"))
                                        .ghost()
                                        .small()
                                        .label("Cancel")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.capture = None;
                                            this.focus.focus(window, cx);
                                            cx.notify();
                                        })),
                                );
                        }
                    }
                } else {
                    controls = controls
                        .invisible()
                        .group_hover(row_id.clone(), Styled::visible)
                        .child(
                            Button::new(format!("reset-shortcut-{id}"))
                                .ghost()
                                .small()
                                .icon(IconName::Undo)
                                .tooltip("Reset shortcut")
                                .accessibility_label(format!(
                                    "Reset shortcut for {}",
                                    shortcut.description()
                                ))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.reset_binding(id, window, cx);
                                })),
                        )
                        .child(
                            Button::new(format!("remove-shortcut-{id}"))
                                .ghost()
                                .small()
                                .icon(IconName::Close)
                                .tooltip("Remove shortcut")
                                .accessibility_label(format!(
                                    "Remove shortcut for {}",
                                    shortcut.description()
                                ))
                                .disabled(shortcut.binding().is_none())
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.remove_binding(id, window, cx);
                                })),
                        );
                }

                let mut binding_label = Button::new(format!("shortcut-binding-{id}"))
                    .small()
                    .rounded_sm()
                    .bg(cx.theme().muted)
                    .font_family(cx.theme().mono_font_family.clone())
                    .accessibility_label(current_binding.clone())
                    .tooltip(format!("Change shortcut for {}", shortcut.description()))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.start_capture(id, window, cx);
                    }));
                if let Some(binding) = shortcut.binding() {
                    binding_label = binding_label
                        .text_color(cx.theme().foreground)
                        .label(keymap::display_binding(binding));
                } else {
                    binding_label = binding_label.child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Icon::new(IconName::TriangleAlert)
                                    .size_4()
                                    .text_color(cx.theme().warning),
                            )
                            .child(
                                div()
                                    .id(format!("shortcut-unbound-warning-{}", shortcut.id()))
                                    .aria_label("No shortcut")
                                    .text_color(cx.theme().warning)
                                    .child("No shortcut"),
                            ),
                    );
                }

                let binding_cell = h_flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(binding_label)
                    .child(controls);

                body = body.child(ShortcutRow {
                    id: row_id,
                    row: TableRow::new()
                        .child(
                            TableCell::new().child(
                                div()
                                    .id(format!("shortcut-action-{}", shortcut.id()))
                                    .aria_label(shortcut.description())
                                    .text_color(cx.theme().foreground)
                                    .child(shortcut.description()),
                            ),
                        )
                        .child(TableCell::new().child(binding_cell)),
                });
            }

            let table = Table::new()
                .accessibility_label(format!("{} shortcuts", group.category))
                .rounded_lg()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground)
                .child(
                    TableHeader::new()
                        .bg(cx.theme().muted)
                        .text_color(cx.theme().muted_foreground)
                        .child(
                            TableRow::new()
                                .child(
                                    TableHead::new().child(
                                        div()
                                            .id(format!(
                                                "shortcuts-action-header-{}",
                                                group.category
                                            ))
                                            .aria_label("Action")
                                            .font_weight(FontWeight::MEDIUM)
                                            .child("Action"),
                                    ),
                                )
                                .child(
                                    TableHead::new().child(
                                        div()
                                            .id(format!(
                                                "shortcuts-binding-header-{}",
                                                group.category
                                            ))
                                            .aria_label("Shortcut")
                                            .font_weight(FontWeight::MEDIUM)
                                            .child("Shortcut"),
                                    ),
                                ),
                        ),
                )
                .child(body);

            groups = groups.child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .id(format!("shortcuts-category-{}", group.category))
                            .aria_label(group.category.clone())
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .child(group.category.clone()),
                    )
                    .child(div().w_full().child(table)),
            );
        }

        groups
    }
}

impl Focusable for ShortcutsView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for ShortcutsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("shortcuts-editor")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .size_full()
            .min_h_0()
            .bg(cx.theme().background)
            .child(
                h_flex()
                    .justify_between()
                    .px_6()
                    .py_4()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .child("Keyboard shortcuts"),
                    )
                    .child(
                        Button::new("reset-all-shortcuts")
                            .ghost()
                            .small()
                            .label("Reset all")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.reset_all(window, cx);
                            })),
                    ),
            )
            .when_some(self.settings_error.clone(), |this, error| {
                this.child(
                    div()
                        .px_6()
                        .py_2()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(error),
                )
            })
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .id("shortcuts-scroll")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .child(
                                div()
                                    .w_full()
                                    .max_w(rems(80.))
                                    .mx_auto()
                                    .px_6()
                                    .py_6()
                                    .child(self.render_tables(cx)),
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .child(Scrollbar::vertical(&self.scroll)),
                    ),
            )
    }
}

fn capture_binding(keystroke: &Keystroke) -> Option<String> {
    let key = keystroke.key.as_str();
    if key.is_empty() || is_modifier_key(key) {
        return None;
    }
    keymap::captured_binding(keystroke)
}

fn is_modifier_key(key: &str) -> bool {
    matches!(
        key,
        "shift" | "ctrl" | "control" | "cmd" | "super" | "alt" | "fn" | "win"
    )
}

fn shortcut_groups(shortcuts: Vec<Shortcut>) -> Vec<ShortcutGroup> {
    let mut groups: Vec<ShortcutGroup> = Vec::new();
    for shortcut in shortcuts {
        match groups.last_mut() {
            Some(group) if group.category.as_str() == shortcut.category() => {
                group.rows.push(shortcut);
            }
            _ => groups.push(ShortcutGroup {
                category: shortcut.category().into(),
                rows: vec![shortcut],
            }),
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, collections::BTreeMap, rc::Rc};

    use gpui_kit::component::Root;
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{AppContext as _, Keystroke, TestAppContext, px, size};

    use super::{CapturePhase, ShortcutsView, capture_binding};
    use crate::app::{actions::NewFile, keymap, settings};

    #[test]
    fn capture_rejects_modifier_only_keys_and_reserved_navigation_keys() {
        assert!(
            capture_binding(&Keystroke {
                key: "shift".into(),
                ..Default::default()
            })
            .is_none()
        );
        for key in ["tab", "escape", "enter"] {
            assert!(capture_binding(&Keystroke::parse(key).unwrap()).is_none());
        }
        assert_eq!(
            capture_binding(&Keystroke::parse("secondary-q").unwrap()),
            Some("secondary-q".into())
        );
    }

    #[gpui_kit::test]
    fn capture_interceptor_suppresses_app_actions_until_save_or_cancel(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        settings::set_shortcut_overrides(BTreeMap::new()).unwrap();
        let dispatched = Rc::new(Cell::new(false));
        let observed = dispatched.clone();
        cx.update(|cx| {
            keymap::register(cx);
            cx.on_action(move |_: &NewFile, _| observed.set(true));
        });
        let mut shortcuts = None;
        let handle = cx.open_window(size(px(840.), px(600.)), |window, cx| {
            let view = cx.new(ShortcutsView::new);
            shortcuts = Some(view.clone());
            Root::new(view, window, cx)
        });
        let shortcuts = shortcuts.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("shortcut-binding-quit", cx);
            window.press("secondary-n", cx);
            assert!(!dispatched.get());
            assert!(matches!(
                shortcuts.read(cx).capture.as_ref().map(|capture| &capture.phase),
                Some(CapturePhase::Proposed(binding)) if binding == "secondary-n"
            ));
            window.press("escape", cx);
            assert!(shortcuts.read(cx).capture.is_none());
            window.press("secondary-n", cx);
            assert!(dispatched.get());

            dispatched.set(false);
            window.click("shortcut-binding-quit", cx);
            window.press("secondary-n", cx);
            assert!(!dispatched.get());
            window.click("save-shortcut-quit", cx);
            assert!(shortcuts.read(cx).capture.is_none());
            assert_eq!(
                shortcuts.read(cx).registry.binding_for("quit"),
                Some("secondary-n")
            );
            assert_eq!(shortcuts.read(cx).registry.binding_for("new-note"), None);
        })
        .unwrap();
        settings::set_shortcut_overrides(BTreeMap::new()).unwrap();
    }
}
