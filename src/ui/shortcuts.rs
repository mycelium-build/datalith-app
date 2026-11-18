//! Shortcut reference and remapping controls in a dedicated workspace tab.

use gpui_kit::base::actions::{
    SelectDown, SelectFirst, SelectLast, SelectPageDown, SelectPageUp, SelectUp,
};
use gpui_kit::component::{
    ActiveTheme, ChildElement, Disableable, Icon, IconName, Sizable, Size,
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
    scroll::Scrollbar,
    table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow},
    tooltip::Tooltip,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, Context, FocusHandle, Focusable, FontWeight, InteractiveElement as _, IntoElement,
    KeyBinding, Keystroke, KeystrokeEvent, ParentElement, Render, RenderOnce, ScrollHandle,
    SharedString, StatefulInteractiveElement as _, Styled, Subscription, Window, div, point, px,
    rems,
};

use crate::app::keymap::{self, Shortcut, ShortcutRegistry, ShortcutZone};

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

const SHORTCUTS_TAB_CONTEXT: &str = "ShortcutsTab";

/// Binds the tab's scrolling keys, which aren't customizable.
pub fn bind_keys(cx: &mut App) {
    let context = Some(SHORTCUTS_TAB_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("down", SelectDown, context),
        KeyBinding::new("up", SelectUp, context),
        KeyBinding::new("pagedown", SelectPageDown, context),
        KeyBinding::new("pageup", SelectPageUp, context),
        KeyBinding::new("home", SelectFirst, context),
        KeyBinding::new("end", SelectLast, context),
    ]);
}

#[derive(Clone, Copy)]
enum ScrollStep {
    LineDown,
    LineUp,
    PageDown,
    PageUp,
    Top,
    Bottom,
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
    /// Where another binding wins each shortcut's keys, by shortcut id.
    shadowed: Vec<(&'static str, Vec<&'static str>)>,
    capture: Option<ShortcutCapture>,
    settings_error: Option<String>,
    _capture_subscription: Subscription,
}

impl ShortcutsView {
    pub(crate) fn new(cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        let registry = ShortcutRegistry::new();
        let groups = shortcut_groups(registry.shortcuts());
        let shadowed = shadowed_places(&registry, cx);
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
            shadowed,
            capture: None,
            settings_error: None,
            _capture_subscription: capture_subscription,
        }
    }

    #[allow(
        clippy::arithmetic_side_effects,
        reason = "floating-point pixel arithmetic; scroll offsets are clamped to bounds"
    )]
    fn scroll(&self, step: ScrollStep, window: &Window, cx: &mut Context<Self>) {
        let current = self.scroll.offset();
        let max = self.scroll.max_offset().y;
        let line = window.rem_size() * 2.0;
        let page = self.scroll.bounds().size.height * 0.85;
        let next = match step {
            ScrollStep::LineDown => current.y - line,
            ScrollStep::LineUp => current.y + line,
            ScrollStep::PageDown => current.y - page,
            ScrollStep::PageUp => current.y + page,
            ScrollStep::Top => px(0.),
            ScrollStep::Bottom => -max,
        };
        self.scroll
            .set_offset(point(current.x, next.clamp(-max, px(0.))));
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
                let zone = self
                    .groups
                    .iter()
                    .flat_map(|group| &group.rows)
                    .find(|shortcut| shortcut.id() == capture.id)
                    .map_or(ShortcutZone::Global, Shortcut::zone);
                capture.phase = match capture_binding(&event.keystroke) {
                    Some(binding) if zone.accepts(&binding) => CapturePhase::Proposed(binding),
                    Some(_) => CapturePhase::Invalid(MODIFIER_NEEDED_IN_NOTES.into()),
                    None => CapturePhase::Invalid("That key cannot be used. Try another.".into()),
                };
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

    fn cancel_capture(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.capture = None;
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

    fn set_zone(
        &mut self,
        id: &'static str,
        zone: ShortcutZone,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_change(|registry, cx| registry.set_zone(id, zone, cx), window, cx);
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
        self.shadowed = shadowed_places(&self.registry, cx);
        self.capture = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn render_tables(&self, cx: &Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_5()
            .children(self.groups.iter().map(|group| self.render_group(group, cx)))
    }

    fn render_group(&self, group: &ShortcutGroup, cx: &Context<Self>) -> impl IntoElement {
        let body = TableBody::new().children(
            group
                .rows
                .iter()
                .map(|shortcut| self.render_shortcut_row(shortcut, cx)),
        );
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
                                        .id(format!("shortcuts-action-header-{}", group.category))
                                        .aria_label("Action")
                                        .font_weight(FontWeight::MEDIUM)
                                        .child("Action"),
                                ),
                            )
                            .child(
                                TableHead::new().child(
                                    div()
                                        .id(format!("shortcuts-binding-header-{}", group.category))
                                        .aria_label("Shortcut")
                                        .font_weight(FontWeight::MEDIUM)
                                        .child("Shortcut"),
                                ),
                            )
                            .child(
                                TableHead::new().child(
                                    div()
                                        .id(format!("shortcuts-zone-header-{}", group.category))
                                        .aria_label("Active in")
                                        .font_weight(FontWeight::MEDIUM)
                                        .child("Active in"),
                                ),
                            ),
                    ),
            )
            .child(body);
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
            .child(div().w_full().child(table))
    }

    fn render_shortcut_row(&self, shortcut: &Shortcut, cx: &Context<Self>) -> ShortcutRow {
        let id = shortcut.id();
        let row_id: SharedString = format!("shortcut-row-{id}").into();
        let binding_cell = h_flex()
            .items_center()
            .justify_between()
            .gap_3()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(Self::render_binding_button(shortcut, cx))
                    .children(self.render_shadowed(shortcut, cx)),
            )
            .child(self.render_shortcut_controls(shortcut, &row_id, cx));
        ShortcutRow {
            id: row_id,
            row: TableRow::new()
                .child(
                    TableCell::new().child(
                        div()
                            .id(format!("shortcut-action-{id}"))
                            .aria_label(shortcut.description())
                            .text_color(cx.theme().foreground)
                            .child(shortcut.description()),
                    ),
                )
                .child(TableCell::new().child(binding_cell))
                .child(TableCell::new().child(Self::render_zone(shortcut, cx))),
        }
    }

    /// An icon whose tooltip lists where another binding wins the keys.
    fn render_shadowed(&self, shortcut: &Shortcut, cx: &Context<Self>) -> Option<impl IntoElement> {
        let places = self
            .shadowed
            .iter()
            .find(|(id, _)| *id == shortcut.id())
            .map(|(_, places)| places)
            .filter(|places| !places.is_empty())?;
        let description = format!("Inactive {}", places.join(", "));
        Some(
            div()
                .id(format!("shortcut-shadowed-{}", shortcut.id()))
                .aria_label(description.clone())
                .tooltip(move |window, cx| Tooltip::new(description.clone()).build(window, cx))
                .child(
                    Icon::new(IconName::Info)
                        .size_4()
                        .text_color(cx.theme().muted_foreground),
                ),
        )
    }

    /// The zone, as plain text when the action can act in only one.
    fn render_zone(shortcut: &Shortcut, cx: &Context<Self>) -> gpui_kit::AnyElement {
        let id = shortcut.id();
        let current = shortcut.zone();
        if let [only] = shortcut.zones() {
            return div()
                .id(format!("shortcut-zone-{id}"))
                .aria_label(only.label())
                .text_color(cx.theme().muted_foreground)
                .child(only.label())
                .into_any_element();
        }
        let zones = shortcut.zones();
        let keys = shortcut.binding().map(str::to_owned);
        let view = cx.weak_entity();
        // Pull the ghost button back by its padding so its label lines up
        // with the header and the plain-text zones.
        Button::new(format!("shortcut-zone-{id}"))
            .ghost()
            .small()
            .ml_neg_2()
            .label(current.label())
            .dropdown_caret(true)
            .accessibility_label(format!(
                "{} is active in {}",
                shortcut.description(),
                current.label()
            ))
            .dropdown_menu(move |mut menu, _, _| {
                for &zone in zones {
                    let view = view.clone();
                    let accepted = keys.as_deref().is_none_or(|keys| zone.accepts(keys));
                    menu = menu.item(
                        PopupMenuItem::new(zone.label())
                            .checked(zone == current)
                            .disabled(!accepted)
                            .on_click(move |_, window, cx| {
                                let _ = view.update(cx, |this, cx| {
                                    this.set_zone(id, zone, window, cx);
                                });
                            }),
                    );
                }
                menu
            })
            .into_any_element()
    }

    fn render_binding_button(shortcut: &Shortcut, cx: &Context<Self>) -> Button {
        let id = shortcut.id();
        let current_binding = keymap::display_binding_or_unbound(shortcut.binding());
        let mut binding_label = Button::new(format!("shortcut-binding-{id}"))
            .small()
            .rounded_sm()
            .bg(cx.theme().muted)
            .font_family(cx.theme().mono_font_family.clone())
            .accessibility_label(current_binding)
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
        binding_label
    }

    fn render_shortcut_controls(
        &self,
        shortcut: &Shortcut,
        row_id: &SharedString,
        cx: &Context<Self>,
    ) -> gpui_kit::Div {
        let id = shortcut.id();
        if let Some(capture) = self.capture.as_ref().filter(|capture| capture.id == id) {
            return Self::render_capture_controls(capture, cx);
        }
        h_flex()
            .items_center()
            .gap_2()
            .invisible()
            .group_hover(row_id.clone(), Styled::visible)
            .child(
                Button::new(format!("reset-shortcut-{id}"))
                    .ghost()
                    .small()
                    .icon(IconName::Undo)
                    .tooltip("Reset shortcut")
                    .accessibility_label(format!("Reset shortcut for {}", shortcut.description()))
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
                    .accessibility_label(format!("Remove shortcut for {}", shortcut.description()))
                    .disabled(shortcut.binding().is_none())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.remove_binding(id, window, cx);
                    })),
            )
    }

    fn render_capture_controls(capture: &ShortcutCapture, cx: &Context<Self>) -> gpui_kit::Div {
        let id = capture.id;
        let mut controls = h_flex().items_center().gap_2();
        match &capture.phase {
            CapturePhase::Listening => {
                controls = controls.child(
                    div()
                        .id(format!("shortcut-capture-status-{id}"))
                        .aria_label("Press a shortcut")
                        .text_color(cx.theme().muted_foreground)
                        .child("Press a shortcut"),
                );
            }
            CapturePhase::Proposed(binding) => {
                controls = controls
                    .child(
                        div()
                            .id(format!("shortcut-proposed-{id}"))
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
                                this.cancel_capture(window, cx);
                            })),
                    );
            }
            CapturePhase::Invalid(message) => {
                controls = controls
                    .child(
                        div()
                            .id(format!("shortcut-capture-error-{id}"))
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
                                this.cancel_capture(window, cx);
                            })),
                    );
            }
        }
        controls
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
            .key_context(SHORTCUTS_TAB_CONTEXT)
            .on_action(cx.listener(|this, _: &SelectDown, window, cx| {
                this.scroll(ScrollStep::LineDown, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectUp, window, cx| {
                this.scroll(ScrollStep::LineUp, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectPageDown, window, cx| {
                this.scroll(ScrollStep::PageDown, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectPageUp, window, cx| {
                this.scroll(ScrollStep::PageUp, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectFirst, window, cx| {
                this.scroll(ScrollStep::Top, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectLast, window, cx| {
                this.scroll(ScrollStep::Bottom, window, cx);
            }))
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

#[cfg(target_os = "macos")]
const MODIFIER_NEEDED_IN_NOTES: &str = "In notes, add Cmd or Ctrl.";
#[cfg(not(target_os = "macos"))]
const MODIFIER_NEEDED_IN_NOTES: &str = "In notes, add Ctrl or Alt.";

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

fn shadowed_places(
    registry: &ShortcutRegistry,
    cx: &App,
) -> Vec<(&'static str, Vec<&'static str>)> {
    registry
        .shortcuts()
        .iter()
        .map(|shortcut| (shortcut.id(), registry.shadowed_places(shortcut.id(), cx)))
        .collect()
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
