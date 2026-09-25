//! Read-only keyboard shortcut reference in a dedicated workspace tab.

use gpui_kit::base::{StyledExt as _, TestSupportExt as _};
use gpui_kit::component::{
    ActiveTheme, Sizable as _,
    scroll::Scrollbar,
    table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow},
    v_flex,
};
use gpui_kit::{
    App, Context, FocusHandle, Focusable, InteractiveElement as _, IntoElement, KeyDownEvent,
    Keystroke, ParentElement, Render, ScrollHandle, SharedString, StatefulInteractiveElement as _,
    Styled, Window, div, point, px, rems,
};

struct ShortcutGroup {
    category: SharedString,
    rows: Vec<(SharedString, SharedString)>, // action, displayed binding
}

pub struct ShortcutsView {
    focus: FocusHandle,
    scroll: ScrollHandle,
    groups: Vec<ShortcutGroup>,
}

impl ShortcutsView {
    pub(crate) fn new(cx: &Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            groups: shortcut_groups(),
        }
    }

    // Pixel arithmetic is bounded to the scroll handle's measured content extent.
    #[allow(clippy::arithmetic_side_effects)]
    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // Let the workspace handle modified keys (tab selection, close, commands, etc.).
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

    fn render_table(&self, cx: &App) -> Table {
        let mut table = Table::new()
            .small()
            .accessibility_label("Keyboard shortcuts")
            .child(
                TableHeader::new().child(
                    TableRow::new()
                        .child(
                            TableHead::new().col_span(2).child(
                                div()
                                    .id("shortcuts-action-header")
                                    .test_support()
                                    .aria_label("Action")
                                    .child("Action"),
                            ),
                        )
                        .child(
                            TableHead::new().child(
                                div()
                                    .id("shortcuts-binding-header")
                                    .test_support()
                                    .aria_label("Shortcut")
                                    .child("Shortcut"),
                            ),
                        ),
                ),
            );

        for group in &self.groups {
            let mut body = TableBody::new().child(
                TableRow::new().child(
                    TableHead::new()
                        .col_span(3)
                        .pt_4()
                        .pb_1()
                        .font_bold()
                        .text_color(cx.theme().foreground)
                        .child(
                            div()
                                .id(format!("shortcuts-category-{}", group.category))
                                .test_support()
                                .aria_label(group.category.clone())
                                .child(group.category.clone()),
                        ),
                ),
            );
            for (action, binding) in &group.rows {
                body = body.child(
                    TableRow::new()
                        .child(
                            TableCell::new().col_span(2).child(
                                div()
                                    .id(format!("shortcut-action-{}-{binding}", group.category))
                                    .test_support()
                                    .aria_label(action.clone())
                                    .child(action.clone()),
                            ),
                        )
                        .child(
                            TableCell::new()
                                .font_family(cx.theme().mono_font_family.clone())
                                .text_color(cx.theme().muted_foreground)
                                .child(
                                    div()
                                        .id(format!(
                                            "shortcut-binding-{}-{binding}",
                                            group.category
                                        ))
                                        .test_support()
                                        .aria_label(binding.clone())
                                        .child(binding.clone()),
                                ),
                        ),
                );
            }
            table = table.child(body);
        }
        table
    }
}

impl Focusable for ShortcutsView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for ShortcutsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("shortcuts-editor")
            .test_support()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .size_full()
            .min_h_0()
            .border_1()
            .border_color(if self.focus.is_focused(window) {
                cx.theme().ring
            } else {
                cx.theme().background
            })
            .bg(cx.theme().background)
            .child(
                div()
                    .p_4()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .text_lg()
                    .child("Keyboard shortcuts"),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .id("shortcuts-scroll")
                            .test_support()
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .child(
                                div()
                                    .w_full()
                                    .max_w(rems(52.))
                                    .mx_auto()
                                    .px_4()
                                    .pb_4()
                                    .child(self.render_table(cx)),
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

/// Collapse only adjacent digit bindings with the same modifiers and no missing digit.
/// The input is the registration list, so this never synthesizes a shortcut.
fn consecutive_digit(previous: &str, next: &str) -> bool {
    let (Ok(previous), Ok(next)) = (Keystroke::parse(previous), Keystroke::parse(next)) else {
        return false;
    };
    previous.modifiers == next.modifiers
        && previous.key.len() == 1
        && next.key.len() == 1
        && previous
            .key
            .parse::<u8>()
            .ok()
            .and_then(|digit| digit.checked_add(1))
            .is_some_and(|digit| next.key.parse::<u8>() == Ok(digit))
}

fn shortcut_groups() -> Vec<ShortcutGroup> {
    struct Run {
        category: &'static str,
        action: &'static str,
        first: &'static str,
        last: &'static str,
    }

    let mut runs: Vec<Run> = Vec::new();
    for (category, keys, action) in crate::app::keymap::shortcut_descriptions() {
        if let Some(run) = runs.last_mut()
            && run.category == category
            && run.action == action
            && consecutive_digit(run.last, keys)
        {
            run.last = keys;
        } else {
            runs.push(Run {
                category,
                action,
                first: keys,
                last: keys,
            });
        }
    }

    let mut groups: Vec<ShortcutGroup> = Vec::new();
    for run in runs {
        let first = crate::app::keymap::display_binding(run.first);
        let binding = if run.first == run.last {
            first
        } else {
            format!(
                "{first} … {}",
                crate::app::keymap::display_binding(run.last)
            )
        };
        match groups.last_mut() {
            Some(group) if group.category.as_str() == run.category => {
                group.rows.push((run.action.into(), binding.into()));
            }
            _ => groups.push(ShortcutGroup {
                category: run.category.into(),
                rows: vec![(run.action.into(), binding.into())],
            }),
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::{consecutive_digit, shortcut_groups};
    use crate::app::keymap;
    use gpui_kit::component::Root;
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{AppContext as _, Role, ScrollDelta, TestAppContext, point, px, size};

    #[test]
    fn collapses_only_registered_contiguous_digit_bindings() {
        assert!(consecutive_digit("secondary-1", "secondary-2"));
        assert!(!consecutive_digit("secondary-1", "secondary-3"));
        assert!(!consecutive_digit("secondary-1", "shift-secondary-2"));
        assert!(!consecutive_digit("secondary-a", "secondary-b"));

        let secondary = if cfg!(target_os = "macos") {
            "⌘"
        } else {
            "ctrl-"
        };
        let tabs = shortcut_groups()
            .into_iter()
            .find(|group| group.category == "Tabs")
            .expect("registered tab shortcuts");
        assert!(tabs.rows.iter().any(|(action, keys)| {
            action == "Select tab" && keys == &format!("{secondary}1 … {secondary}8")
        }));
    }

    #[gpui_kit::test]
    fn shortcut_reference_has_aligned_columns_and_scrolls_from_keyboard(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let mut shortcuts = None;
        let handle = cx.open_window(size(px(640.), px(360.)), |window, cx| {
            let view = cx.new(|cx| super::ShortcutsView::new(cx));
            let focus = view.read(cx).focus.clone();
            focus.focus(window, cx);
            shortcuts = Some(view.clone());
            Root::new(view, window, cx)
        });
        let shortcuts = shortcuts.expect("shortcut reference view");

        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("shortcuts-editor").focused(), Some(true));
            assert_eq!(window.find(("table", 0_usize)).role(), Some(Role::Table));
            assert_eq!(
                window.find("shortcuts-action-header").label(),
                Some("Action")
            );
            assert_eq!(
                window.find("shortcuts-binding-header").label(),
                Some("Shortcut")
            );
            assert_eq!(window.find("shortcuts-category-File").label(), Some("File"));

            let binding = keymap::display_binding("secondary-n");
            let action = window.find(format!("shortcut-action-File-{binding}"));
            let key = window.find(format!("shortcut-binding-File-{binding}"));
            assert_eq!(action.label(), Some("New note"));
            assert_eq!(key.label(), Some(binding.as_str()));
            assert_eq!(
                action.bounds().left(),
                window.find("shortcuts-action-header").bounds().left()
            );
            assert_eq!(
                key.bounds().left(),
                window.find("shortcuts-binding-header").bounds().left()
            );

            let before = shortcuts.read(cx).scroll.offset().y;
            window.press("pagedown", cx);
            assert!(shortcuts.read(cx).scroll.offset().y < before);
            window.press("end", cx);
            assert!(window.find("shortcuts-category-Help").visible());
            window.press("home", cx);
            assert_eq!(shortcuts.read(cx).scroll.offset().y, px(0.));
            assert!(window.find("shortcuts-category-File").visible());
            window.scroll(
                "shortcuts-scroll",
                ScrollDelta::Pixels(point(px(0.), px(-80.))),
                cx,
            );
            assert!(shortcuts.read(cx).scroll.offset().y < px(0.));
            assert_eq!(window.find("shortcuts-editor").focused(), Some(true));
        })
        .expect("shortcuts window");
    }
}
