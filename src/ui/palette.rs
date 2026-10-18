use std::path::PathBuf;

use gpui_kit::component::{
    ActiveTheme, Icon, VirtualListScrollHandle, h_flex,
    input::{Input, InputEvent, InputState},
    v_flex, v_virtual_list,
};
use gpui_kit::{
    AnyElement, AppContext, Context, ElementId, Entity, Focusable, FontWeight, HighlightStyle,
    Hsla, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels, ScrollStrategy,
    SharedString, Size, StatefulInteractiveElement, Styled, StyledText, Subscription, Window, div,
    px, size,
};

use crate::ui::DatalithView;
use crate::vault::VaultCatalog;
use crate::vault::path::display_name;
use crate::vault::search::{SearchHit, picker};

const BORDER_WIDTH: f32 = 2.0;
const MIN_SEARCH_QUERY_LENGTH: usize = 3;
const PALETTE_WIDTH: f32 = 600.0;
const SEARCH_ITEM_HEIGHT: f32 = 56.0;
const QUICK_SWITCHER_ITEM_HEIGHT: f32 = 28.0;
const PALETTE_MAX_HEIGHT: f32 = 400.0;
const SNIPPET_INDENT: f32 = 20.0;
const SCORE_HIGH_RATIO: f32 = 0.66;
const SCORE_MEDIUM_RATIO: f32 = 0.33;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PaletteKind {
    Search,
    QuickSwitcher,
}

#[derive(Clone)]
pub struct Palette {
    pub(crate) kind: PaletteKind,
    switching_from: Option<PaletteKind>,
    pub(crate) open: bool,
    pub(crate) needs_focus: bool,
    pub(crate) input: Entity<InputState>,
    pub(crate) selected: Option<usize>,
    pub(crate) search_query: SharedString,
    pub(crate) search_results: Vec<SearchHit>,
    pub(crate) qs_query: SharedString,
    pub(crate) quick_switcher_entries: Vec<picker::QuickSwitcherEntry>,
    item_sizes: Vec<Size<Pixels>>,
    scroll_handle: VirtualListScrollHandle,
}

fn score_color(cx: &Context<DatalithView>, score: f32, max_score: f32) -> Hsla {
    let ratio = if max_score > 0.0 {
        (score / max_score).clamp(0.0, 1.0)
    } else {
        0.0
    };
    if ratio >= SCORE_HIGH_RATIO {
        cx.theme().success
    } else if ratio >= SCORE_MEDIUM_RATIO {
        cx.theme().warning
    } else {
        cx.theme().info
    }
}

fn score_badge(score: f32, color: Hsla) -> impl IntoElement {
    div()
        .px_1p5()
        .py_0p5()
        .rounded_sm()
        .flex_shrink_0()
        .bg(color.opacity(0.15))
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .child(format!("{score:.2}"))
}

fn snippet_highlight(cx: &Context<DatalithView>) -> HighlightStyle {
    HighlightStyle {
        background_color: Some(cx.theme().warning.opacity(0.3)),
        font_weight: Some(FontWeight::SEMIBOLD),
        ..Default::default()
    }
}

fn render_search_hit(
    view: &DatalithView,
    hit: &SearchHit,
    index: usize,
    selected: bool,
    max_score: f32,
    cx: &Context<DatalithView>,
) -> AnyElement {
    let file_name = display_name(&hit.path).to_string();
    let file_icon = view.registry.config_for(Some(hit.path.as_path())).icon;
    let bg = if selected {
        cx.theme().muted
    } else {
        Hsla::default()
    };
    let path = hit.path.clone();
    let score = hit.score;
    let color = score_color(cx, score, max_score);
    let highlight = snippet_highlight(cx);
    let highlights: Vec<_> = hit
        .highlights
        .iter()
        .cloned()
        .map(|range| (range, highlight))
        .collect();
    let snippet = StyledText::new(hit.snippet.clone()).with_highlights(highlights);

    div()
        .px_2()
        .py_1()
        .h(px(SEARCH_ITEM_HEIGHT))
        .bg(bg)
        .hover(|s| s.bg(cx.theme().muted))
        .cursor_pointer()
        .id(ElementId::Name(format!("result-{index}").into()))
        .on_click(cx.listener(move |view, _, window, cx| {
            view.palette.close();
            view.open_file(path.clone(), false, window, cx);
        }))
        .child(
            v_flex()
                .gap_0p5()
                .w_full()
                .min_w_0()
                .justify_center()
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .w_full()
                        .min_w_0()
                        .child(Icon::new(file_icon).size_3())
                        .child(div().flex_1().min_w_0().truncate().child(file_name))
                        .child(score_badge(score, color)),
                )
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .pl(px(SNIPPET_INDENT))
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .truncate()
                        .child(snippet),
                ),
        )
        .into_any_element()
}

impl Palette {
    #[must_use]
    pub(crate) fn new(window: &mut Window, cx: &mut Context<DatalithView>) -> Self {
        Self {
            kind: PaletteKind::Search,
            open: false,
            needs_focus: false,
            switching_from: None,
            search_query: SharedString::default(),
            qs_query: SharedString::default(),
            input: cx.new(|cx| InputState::new(window, cx).placeholder("Search files...")),
            selected: None,
            scroll_handle: VirtualListScrollHandle::new(),
            item_sizes: Vec::new(),
            search_results: Vec::new(),
            quick_switcher_entries: Vec::new(),
        }
    }

    pub(crate) fn open_as(&mut self, kind: PaletteKind) {
        self.switching_from = (self.kind != kind).then_some(self.kind);
        self.kind = kind;
        self.open = true;
        self.needs_focus = true;
        self.selected = if kind == PaletteKind::QuickSwitcher {
            Some(0)
        } else {
            None
        };
        self.item_sizes.clear();
    }

    pub(crate) const fn close(&mut self) {
        self.open = false;
    }

    pub(crate) fn search(&mut self, catalog: Option<&VaultCatalog>, query: &SharedString) {
        let query = query.trim().to_string();
        self.search_results = if query.len() < MIN_SEARCH_QUERY_LENGTH {
            Vec::new()
        } else {
            catalog
                .map(|catalog| catalog.search(&query))
                .unwrap_or_default()
        };
        self.item_sizes =
            vec![size(px(PALETTE_WIDTH), px(SEARCH_ITEM_HEIGHT)); self.search_results.len()];
    }

    pub(crate) fn refresh_quick_switcher(
        &mut self,
        catalog: Option<&VaultCatalog>,
        open_files: &[PathBuf],
        query: &SharedString,
    ) {
        let all_files = catalog
            .map(|catalog| picker::collect_from_paths(catalog.paths()))
            .unwrap_or_default();
        self.quick_switcher_entries = picker::filter(&all_files, open_files, query);
        self.item_sizes = vec![
            size(px(PALETTE_WIDTH), px(QUICK_SWITCHER_ITEM_HEIGHT));
            self.quick_switcher_entries.len()
        ];
    }

    pub(crate) fn refresh(&mut self, catalog: Option<&VaultCatalog>, open_files: &[PathBuf]) {
        match self.kind {
            PaletteKind::Search => {
                let query = self.search_query.clone();
                self.search(catalog, &query);
            }
            PaletteKind::QuickSwitcher => {
                let query = self.qs_query.clone();
                self.refresh_quick_switcher(catalog, open_files, &query);
            }
        }
    }

    pub(crate) fn scroll_to(&self, index: usize) {
        self.scroll_handle
            .scroll_to_item(index, ScrollStrategy::Nearest);
    }

    fn render_results(&self, cx: &Context<DatalithView>) -> impl IntoElement + use<> {
        let entity = cx.entity();
        let kind = self.kind;
        let item_sizes = self.item_sizes.clone();
        let max_score = self
            .search_results
            .iter()
            .map(|hit| hit.score)
            .fold(0.0_f32, f32::max);

        v_virtual_list(
            entity,
            "palette-results",
            item_sizes.into(),
            move |view, visible_range, _, cx| {
                let selected_idx = view.palette.selected;
                visible_range
                    .map(move |i| match kind {
                        PaletteKind::Search => {
                            let Some(hit) = view.palette.search_results.get(i) else {
                                return div().into_any_element();
                            };
                            render_search_hit(view, hit, i, Some(i) == selected_idx, max_score, cx)
                        }
                        PaletteKind::QuickSwitcher => {
                            let Some(entry) = view.palette.quick_switcher_entries.get(i) else {
                                return div().into_any_element();
                            };
                            let bg = if Some(i) == selected_idx {
                                cx.theme().muted
                            } else {
                                gpui_kit::Hsla::default()
                            };
                            let path = entry.path.clone();
                            let name = entry.name.clone();
                            let file_icon = view.registry.config_for(Some(&path)).icon;
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
                                .h(px(QUICK_SWITCHER_ITEM_HEIGHT))
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
                                        .child(Icon::new(file_icon).size_3())
                                        .child(name)
                                        .child(div().flex_1())
                                        .children(open_label),
                                )
                                .into_any_element()
                        }
                    })
                    .collect()
            },
        )
        .track_scroll(&self.scroll_handle)
        .h(px(PALETTE_MAX_HEIGHT))
    }

    pub(crate) fn render_overlay(&self, cx: &Context<DatalithView>) -> impl IntoElement + use<> {
        let input = self.input.clone();

        div()
            .absolute()
            .inset_0()
            .bg(gpui_kit::black().opacity(0.3))
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
                    .w(px(PALETTE_WIDTH))
                    .bg(cx.theme().background)
                    .border(px(BORDER_WIDTH))
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
                                            PaletteKind::Search => {
                                                view.palette.search_results.len()
                                            }
                                            PaletteKind::QuickSwitcher => {
                                                view.palette.quick_switcher_entries.len()
                                            }
                                        };
                                        if count > 0
                                            && let Some(next) = picker::nav_idx(
                                                key == "down",
                                                view.palette.selected,
                                                count,
                                            )
                                        {
                                            view.palette.selected = Some(next);
                                            view.palette.scroll_to(next);
                                        }
                                        cx.notify();
                                    }
                                },
                            ))
                            .child(
                                div()
                                    .border_b(px(BORDER_WIDTH))
                                    .border_color(cx.theme().border)
                                    .child(Input::new(&input).appearance(false)),
                            )
                            .child(self.render_palette_body(cx)),
                    ),
            )
    }

    fn render_palette_body(&self, cx: &Context<DatalithView>) -> AnyElement {
        let (query, has_results) = match self.kind {
            PaletteKind::Search => (self.search_query.trim(), !self.search_results.is_empty()),
            PaletteKind::QuickSwitcher => (
                self.qs_query.trim(),
                !self.quick_switcher_entries.is_empty(),
            ),
        };
        if query.is_empty() {
            return match self.kind {
                PaletteKind::Search => {
                    Self::render_body_hint(cx, "Start typing to search your vault")
                        .into_any_element()
                }
                PaletteKind::QuickSwitcher if has_results => {
                    self.render_results(cx).into_any_element()
                }
                PaletteKind::QuickSwitcher => {
                    Self::render_body_hint(cx, "No files open in this vault").into_any_element()
                }
            };
        }
        if self.kind == PaletteKind::Search && query.len() < MIN_SEARCH_QUERY_LENGTH {
            return Self::render_body_hint(cx, "Keep typing to search").into_any_element();
        }
        if !has_results {
            return Self::render_body_hint(cx, "No files match your query").into_any_element();
        }
        self.render_results(cx).into_any_element()
    }

    fn render_body_hint(cx: &Context<DatalithView>, hint: &'static str) -> impl IntoElement {
        v_flex()
            .h(px(PALETTE_MAX_HEIGHT))
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(hint),
            )
    }

    pub(crate) fn input_subscription(
        input: &Entity<InputState>,
        window: &Window,
        cx: &mut Context<DatalithView>,
    ) -> Subscription {
        cx.subscribe_in(
            input,
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
                                view.palette.search_query = value.clone();
                                view.palette.search(view.vault_catalog.as_ref(), &value);
                            }
                            PaletteKind::QuickSwitcher => {
                                view.palette.qs_query = value.clone();
                                let open_paths = view.tabs.open_paths();
                                view.palette.refresh_quick_switcher(
                                    view.vault_catalog.as_ref(),
                                    &open_paths,
                                    &value,
                                );
                            }
                        }
                        view.palette.selected = match view.palette.kind {
                            PaletteKind::Search if !trimmed_is_empty => None,
                            PaletteKind::QuickSwitcher
                                if view.palette.quick_switcher_entries.is_empty() =>
                            {
                                None
                            }
                            _ => Some(0),
                        };
                        cx.notify();
                    }
                    InputEvent::PressEnter { secondary, .. } => {
                        let open_path = match view.palette.kind {
                            PaletteKind::Search => view
                                .palette
                                .selected
                                .and_then(|i| view.palette.search_results.get(i))
                                .map(|hit| hit.path.clone()),
                            PaletteKind::QuickSwitcher => view
                                .palette
                                .selected
                                .and_then(|i| view.palette.quick_switcher_entries.get(i))
                                .map(|e| e.path.clone()),
                        };
                        if let Some(path) = open_path {
                            view.palette.close();
                            view.open_file(path, *secondary, window, cx);
                        }
                        cx.notify();
                    }
                    _ => {}
                }
            },
        )
    }

    pub(crate) fn focus_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<DatalithView>,
        vault_catalog: Option<&VaultCatalog>,
        open_files: &[PathBuf],
    ) {
        let placeholder = match self.kind {
            PaletteKind::Search => "Search files...",
            PaletteKind::QuickSwitcher => "Switch files...",
        };
        self.input.update(cx, |input, cx| {
            input.set_placeholder(placeholder, window, cx);
        });
        if let Some(from) = self.switching_from.take() {
            let current = self.input.read(cx).value();
            match from {
                PaletteKind::Search => self.search_query = current,
                PaletteKind::QuickSwitcher => self.qs_query = current,
            }
        }

        let restored = match self.kind {
            PaletteKind::Search => self.search_query.clone(),
            PaletteKind::QuickSwitcher => self.qs_query.clone(),
        };
        self.input.update(cx, |input, cx| {
            input.set_value(restored, window, cx);
        });
        match self.kind {
            PaletteKind::Search => {
                let query = self.search_query.clone();
                self.search(vault_catalog, &query);
                if self.search_query.trim().is_empty() {
                    self.selected = Some(0);
                } else {
                    self.selected = None;
                }
            }
            PaletteKind::QuickSwitcher => {
                let query = self.qs_query.clone();
                self.refresh_quick_switcher(vault_catalog, open_files, &query);
                self.selected = if self.quick_switcher_entries.is_empty() {
                    None
                } else {
                    Some(0)
                };
            }
        }
        self.input.focus_handle(cx).focus(window, cx);
        self.needs_focus = false;
    }
}
