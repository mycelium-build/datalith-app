use std::rc::Rc;

use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, IconName, VirtualListScrollHandle, h_flex,
    input::{Input, InputEvent, InputState},
    v_flex, v_virtual_list,
};

use crate::search::SearchResult;

use super::DatalithView;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PaletteKind {
    Search,
    QuickSwitcher,
}

pub struct Palette {
    pub kind: PaletteKind,
    pub open: bool,
    pub needs_focus: bool,
    pub input: Entity<InputState>,
    pub selected: Option<usize>,
    pub scroll_handle: VirtualListScrollHandle,
    pub item_sizes: Rc<Vec<Size<Pixels>>>,
}

impl Palette {
    pub fn new(window: &mut Window, cx: &mut Context<DatalithView>) -> Self {
        Self {
            kind: PaletteKind::Search,
            open: false,
            needs_focus: false,
            input: cx.new(|cx| InputState::new(window, cx).placeholder("Search files...")),
            selected: None,
            scroll_handle: VirtualListScrollHandle::new(),
            item_sizes: Rc::new(Vec::new()),
        }
    }

    pub fn open_as(&mut self, kind: PaletteKind) {
        self.kind = kind;
        self.open = true;
        self.needs_focus = true;
        self.selected = if kind == PaletteKind::QuickSwitcher {
            Some(0)
        } else {
            None
        };
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn set_search_sizes(&mut self, results: &[SearchResult]) {
        self.item_sizes = Rc::new(vec![size(px(600.), px(28.)); results.len()]);
    }

    pub fn set_quick_switcher_sizes(&mut self, count: usize) {
        self.item_sizes = Rc::new(vec![size(px(600.), px(28.)); count]);
    }

    pub fn scroll_to(&mut self, index: usize) {
        self.scroll_handle
            .scroll_to_item(index, ScrollStrategy::Nearest);
    }

    pub fn nav_idx(down: bool, selected: Option<usize>, count: usize) -> usize {
        match (down, selected) {
            (true, Some(i)) if i + 1 < count => i + 1,
            (true, _) => 0,
            (false, Some(i)) if i > 0 => i - 1,
            (false, _) => count - 1,
        }
    }

    fn render_results(&self, cx: &mut Context<DatalithView>) -> impl IntoElement + use<> {
        let entity = cx.entity().clone();
        let kind = self.kind;
        let item_sizes = self.item_sizes.clone();

        v_virtual_list(
            entity,
            "palette-results",
            item_sizes,
            move |view, visible_range, _, cx| {
                let selected_idx = view.palette.selected;
                visible_range
                    .map(move |i| match kind {
                        PaletteKind::Search => {
                            let r = &view.search_results[i];
                            let file_name = r
                                .path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("")
                                .to_string();
                            let bg = if Some(i) == selected_idx {
                                cx.theme().muted
                            } else {
                                gpui::Hsla::default()
                            };
                            let path = r.path.clone();
                            div()
                                .px_2()
                                .py_1()
                                .bg(bg)
                                .hover(|s| s.bg(cx.theme().muted))
                                .cursor_pointer()
                                .id(ElementId::Name(format!("result-{i}").into()))
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    view.palette.close();
                                    view.open_file(path.clone(), false, window, cx);
                                }))
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(Icon::new(IconName::File).size_3())
                                        .child(file_name),
                                )
                        }
                        PaletteKind::QuickSwitcher => {
                            let entry = &view.quick_switcher_entries[i];
                            let bg = if Some(i) == selected_idx {
                                cx.theme().muted
                            } else {
                                gpui::Hsla::default()
                            };
                            let path = entry.path.clone();
                            let name = entry.name.clone();
                            let open_label = if entry.open {
                                Some(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child("open"),
                                )
                            } else {
                                None
                            };
                            div()
                                .px_2()
                                .py_1()
                                .bg(bg)
                                .hover(|s| s.bg(cx.theme().muted))
                                .cursor_pointer()
                                .id(ElementId::Name(format!("qs-{i}").into()))
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    view.palette.close();
                                    view.open_file(path.clone(), false, window, cx);
                                }))
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(Icon::new(IconName::File).size_3())
                                        .child(name)
                                        .child(div().flex_1())
                                        .children(open_label),
                                )
                        }
                    })
                    .collect()
            },
        )
        .track_scroll(&self.scroll_handle)
        .h(px(400.))
    }

    pub fn render_overlay(&self, cx: &mut Context<DatalithView>) -> impl IntoElement + use<> {
        let input = self.input.clone();
        let results = self.render_results(cx);

        div()
            .absolute()
            .inset_0()
            .bg(gpui::black().opacity(0.3))
            .flex()
            .items_center()
            .justify_center()
            .id("palette-backdrop")
            .on_click(cx.listener(|view: &mut DatalithView, _, _, cx| {
                view.palette.close();
                cx.notify();
            }))
            .child(
                div()
                    .w(px(600.))
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .shadow_lg()
                    .id("palette-panel")
                    .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()))
                    .child(
                        v_flex()
                            .overflow_hidden()
                            .on_key_down(cx.listener(
                                |view: &mut DatalithView,
                                 event: &KeyDownEvent,
                                 _: &mut Window,
                                 cx: &mut Context<DatalithView>| {
                                    let key = event.keystroke.key.as_str();
                                    if key == "escape" {
                                        view.palette.close();
                                        cx.notify();
                                        return;
                                    }
                                    if key == "up" || key == "down" {
                                        let count = match view.palette.kind {
                                            PaletteKind::Search => view.search_results.len(),
                                            PaletteKind::QuickSwitcher => {
                                                view.quick_switcher_entries.len()
                                            }
                                        };
                                        if count > 0 {
                                            let next = Palette::nav_idx(
                                                key == "down",
                                                view.palette.selected,
                                                count,
                                            );
                                            view.palette.selected = Some(next);
                                            view.palette.scroll_to(next);
                                        }
                                        cx.notify();
                                    }
                                },
                            ))
                            .child(Input::new(&input))
                            .child(results),
                    ),
            )
    }

    pub fn input_subscription(
        input: Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<DatalithView>,
    ) -> Subscription {
        cx.subscribe_in(
            &input,
            window,
            |view: &mut DatalithView,
             input: &Entity<InputState>,
             event: &InputEvent,
             window: &mut Window,
             cx: &mut Context<DatalithView>| {
                match event {
                    InputEvent::Change => {
                        let value = input.read(cx).value();
                        let trimmed_is_empty = value.trim().is_empty();
                        match view.palette.kind {
                            PaletteKind::Search => {
                                view.search(value);
                            }
                            PaletteKind::QuickSwitcher => {
                                view.filter_quick_switcher(value);
                            }
                        }
                        view.palette.selected = match view.palette.kind {
                            PaletteKind::Search if !trimmed_is_empty => None,
                            PaletteKind::QuickSwitcher
                                if view.quick_switcher_entries.is_empty() =>
                            {
                                None
                            }
                            _ => Some(0),
                        };
                        cx.notify();
                    }
                    InputEvent::PressEnter { .. } => {
                        let open_path = match view.palette.kind {
                            PaletteKind::Search => view
                                .palette
                                .selected
                                .and_then(|i| view.search_results.get(i))
                                .map(|r| r.path.clone()),
                            PaletteKind::QuickSwitcher => view
                                .palette
                                .selected
                                .and_then(|i| view.quick_switcher_entries.get(i))
                                .map(|e| e.path.clone()),
                        };
                        if let Some(path) = open_path {
                            view.palette.close();
                            view.open_file(path, false, window, cx);
                        }
                        cx.notify();
                    }
                    _ => {}
                }
            },
        )
    }

    pub fn clear_and_focus(&mut self, window: &mut Window, cx: &mut Context<DatalithView>) {
        self.input.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        self.input.focus_handle(cx).focus(window, cx);
        self.needs_focus = false;
    }
}
