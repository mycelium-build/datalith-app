use std::ops::Range;
use std::path::PathBuf;

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::input::{EditorState, InputEvent, Rope, RopeExt};
use gpui_kit::{
    Context, Empty, Entity, EntityInputHandler as _, Focusable as _, HighlightStyle,
    InteractiveElement as _, IntoElement, MouseButton, ParentElement, Point, Render, Styled,
    StyledText, Subscription, Window, deferred, div, prelude::FluentBuilder as _, px,
};

use crate::vault::VaultCatalog;
use crate::vault::catalog::WikiLinkMatch;

const MIN_QUERY_CHARS: usize = 3;
const MAX_SUGGESTIONS: usize = 32;
const MAX_VISIBLE_ITEMS: usize = 7;
const POPUP_WIDTH: f32 = 360.0;

struct WikiLink {
    query: String,
    replace_start: usize,
    replace_end: usize,
}

/// Suggests vault files while the user types the target of a `[[wiki link]]`.
pub struct WikiLinkAutocomplete {
    input: Entity<EditorState>,
    source: PathBuf,
    catalog: VaultCatalog,
    items: Vec<WikiLinkMatch>,
    selected: usize,
    replace_range: Range<usize>,
    query: String,
    open: bool,
    _subscription: Subscription,
}

impl WikiLinkAutocomplete {
    pub fn new(
        input: Entity<EditorState>,
        source: PathBuf,
        catalog: VaultCatalog,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription =
            cx.subscribe(&input, |this, input, event: &InputEvent, cx| match event {
                InputEvent::Change => this.refresh(&input, cx),
                InputEvent::Blur | InputEvent::Focus => this.close(cx),
                InputEvent::PressEnter { .. } => {}
            });

        Self {
            input,
            source,
            catalog,
            items: Vec::new(),
            selected: 0,
            replace_range: 0..0,
            query: String::new(),
            open: false,
            _subscription: subscription,
        }
    }

    pub fn set_catalog(&mut self, catalog: VaultCatalog) {
        self.catalog = catalog;
    }

    fn suggestions(&self, query: &str) -> Vec<WikiLinkMatch> {
        self.catalog
            .suggest_wiki_links(query, Some(&self.source), MAX_SUGGESTIONS)
    }

    fn refresh(&mut self, input: &Entity<EditorState>, cx: &mut Context<Self>) {
        let link = {
            let state = input.read(cx);
            wiki_link_at(state.text(), state.cursor())
        };
        let Some(link) = link else {
            self.close(cx);
            return;
        };
        if link.query.chars().count() < MIN_QUERY_CHARS {
            self.close(cx);
            return;
        }

        let items = self.suggestions(&link.query);
        if items.is_empty() {
            self.close(cx);
            return;
        }

        let previous = self
            .items
            .get(self.selected)
            .map(|item| item.target.clone());
        self.selected = previous
            .and_then(|target| items.iter().position(|item| item.target == target))
            .unwrap_or(0);
        self.items = items;
        self.replace_range = link.replace_start..link.replace_end;
        self.query = link.query;
        self.open = true;
        cx.notify();
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        if self.open || !self.items.is_empty() {
            self.open = false;
            self.items.clear();
            self.selected = 0;
            cx.notify();
        }
    }

    pub(super) fn handle_enter(&mut self, window: &Window, cx: &mut Context<Self>) -> bool {
        if !self.open {
            return false;
        }
        self.accept(self.selected, window, cx);
        true
    }

    pub(super) fn handle_up(&mut self, _window: &Window, cx: &mut Context<Self>) -> bool {
        self.move_selection(-1, cx)
    }

    pub(super) fn handle_down(&mut self, _window: &Window, cx: &mut Context<Self>) -> bool {
        self.move_selection(1, cx)
    }

    pub(super) fn handle_escape(&mut self, _window: &Window, cx: &mut Context<Self>) -> bool {
        if !self.open {
            return false;
        }
        self.close(cx);
        true
    }

    pub(super) fn handle_mouse_down(&mut self, _window: &Window, cx: &mut Context<Self>) -> bool {
        if !self.open {
            return false;
        }
        self.close(cx);
        false
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) -> bool {
        if !self.open || self.items.is_empty() {
            return false;
        }

        let last = self.items.len().saturating_sub(1);
        self.selected = if delta < 0 {
            self.selected.checked_sub(1).unwrap_or(last)
        } else if self.selected.saturating_add(1) < self.items.len() {
            self.selected.saturating_add(1)
        } else {
            0
        };
        cx.notify();
        true
    }

    fn accept(&mut self, index: usize, window: &Window, cx: &mut Context<Self>) {
        let Some(item) = self.items.get(index).cloned() else {
            return;
        };
        let range = self.replace_range.clone();
        let new_text = format!("[[{}]]", item.target);
        self.close(cx);

        let input = self.input.clone();
        cx.spawn_in(window, async move |_, cx| {
            let _ = input.update_in(cx, |state, window, cx| {
                let range_utf16 = {
                    let text = state.text();
                    text.offset_to_offset_utf16(range.start)..text.offset_to_offset_utf16(range.end)
                };
                state.replace_text_in_range(Some(range_utf16), &new_text, window, cx);
            });
        })
        .detach();
    }
}

impl Render for WikiLinkAutocomplete {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.input.read(cx).focus_handle(cx).is_focused(window);
        if !self.open || self.items.is_empty() || !focused {
            return Empty.into_any_element();
        }

        let Some(position) = popup_position(&self.input, cx) else {
            return Empty.into_any_element();
        };

        let theme = cx.theme();
        let popover_background = theme.popover;
        let foreground = theme.foreground;
        let muted = theme.muted_foreground;
        let border = theme.border;
        let selected_background = theme.accent.opacity(0.25);
        let hover_background = theme.accent.opacity(0.10);
        let highlight_color = theme.blue;
        let radius = theme.radius;

        let visible = visible_range(self.items.len(), self.selected);
        let (folder_query, name_query) = query_parts(&self.query);

        let mut rows = Vec::with_capacity(visible.len());
        for index in visible {
            let Some(item) = self.items.get(index) else {
                continue;
            };
            let selected = index == self.selected;
            let label_highlights = highlight_in(&item.label, name_query)
                .map(|range| vec![(range, highlight_style(highlight_color))])
                .unwrap_or_default();
            let detail_highlights = item
                .detail
                .as_deref()
                .zip(folder_query)
                .and_then(|(detail, query)| highlight_in(detail, query))
                .map(|range| vec![(range, highlight_style(highlight_color))])
                .unwrap_or_default();

            rows.push(
                div()
                    .id(("wikilink-suggestion", index))
                    .flex()
                    .flex_col()
                    .w_full()
                    .gap(px(1.))
                    .px_2()
                    .py_1()
                    .rounded(radius)
                    .when(selected, |this| this.bg(selected_background))
                    .hover(move |this| this.bg(hover_background))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.accept(index, window, cx);
                        }),
                    )
                    .child(div().w_full().min_w_0().truncate().child(
                        StyledText::new(item.label.clone()).with_highlights(label_highlights),
                    ))
                    .when_some(item.detail.clone(), move |this, detail| {
                        this.child(
                            div()
                                .w_full()
                                .min_w_0()
                                .truncate()
                                .text_xs()
                                .text_color(muted)
                                .child(StyledText::new(detail).with_highlights(detail_highlights)),
                        )
                    }),
            );
        }

        let popup = div()
            .id("wikilink-popup")
            .absolute()
            .left(position.x)
            .top(position.y)
            .w(px(POPUP_WIDTH))
            .occlude()
            .overflow_hidden()
            .bg(popover_background)
            .text_color(foreground)
            .border_1()
            .border_color(border)
            .rounded(radius)
            .shadow_md()
            .p_1()
            .flex()
            .flex_col()
            .gap(px(1.))
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.close(cx);
            }))
            .children(rows);
        #[cfg(test)]
        let popup = popup.debug_selector(|| "wikilink-popup".to_string());

        deferred(popup).into_any_element()
    }
}

fn visible_range(total: usize, selected: usize) -> Range<usize> {
    if total <= MAX_VISIBLE_ITEMS {
        return 0..total;
    }

    let start = selected
        .saturating_sub(MAX_VISIBLE_ITEMS / 2)
        .min(total.saturating_sub(MAX_VISIBLE_ITEMS));
    start..start.saturating_add(MAX_VISIBLE_ITEMS)
}

#[allow(clippy::arithmetic_side_effects)]
fn popup_position(
    input: &Entity<EditorState>,
    cx: &gpui_kit::App,
) -> Option<Point<gpui_kit::Pixels>> {
    let editor = input.read(cx);
    let (cursor_bounds, line_height) = editor.cursor_layout()?;
    Some(
        editor.scroll_offset() + cursor_bounds.origin - editor.input_bounds().origin
            + Point::new(px(-4.), line_height + px(4.)),
    )
}

fn highlight_style(color: gpui_kit::Hsla) -> HighlightStyle {
    HighlightStyle {
        color: Some(color),
        ..Default::default()
    }
}

fn query_parts(query: &str) -> (Option<&str>, &str) {
    query
        .rsplit_once('/')
        .map_or((None, query), |(folder, name)| {
            (Some(folder.rsplit('/').next().unwrap_or(folder)), name)
        })
}

fn highlight_in(text: &str, needle: &str) -> Option<Range<usize>> {
    let needle = needle.trim();
    if needle.is_empty() {
        return None;
    }

    let length = needle.len();
    text.char_indices().find_map(|(start, _)| {
        let end = start.saturating_add(length);
        text.get(start..end)
            .filter(|candidate| candidate.eq_ignore_ascii_case(needle))
            .map(|_| start..end)
    })
}

fn wiki_link_at(text: &Rope, offset: usize) -> Option<WikiLink> {
    if offset > text.len() || !text.is_char_boundary(offset) {
        return None;
    }

    let position = text.offset_to_position(offset);
    let line = usize::try_from(position.line).ok()?;
    let line_start = text.line_start_offset(line);
    let before = text.slice(line_start..offset).to_string();
    let open_index = before.rfind("[[")?;
    let query = before.get(open_index.saturating_add(2)..)?;

    // A closed link or a link that already points at an anchor or label is not being edited any more.
    if query.contains("]]") || query.contains(['|', '#']) {
        return None;
    }

    let replace_start = line_start.checked_add(open_index)?;
    let after = text
        .slice(offset..text.len().min(offset.saturating_add(2)))
        .to_string();
    let replace_end = if after == "]]" {
        offset.saturating_add(2)
    } else {
        offset
    };

    Some(WikiLink {
        query: query.trim().to_string(),
        replace_start,
        replace_end,
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::Duration;

    use super::*;
    use crate::document::file_types::RegisteredFileTypes;

    struct TempVault(PathBuf);

    impl TempVault {
        fn new() -> Self {
            let root = std::env::temp_dir()
                .join(format!("datalith-wikilink-{:032x}", rand::random::<u128>()));
            std::fs::create_dir_all(&root).unwrap();
            for name in ["Source.md", "Alpha.md", "Alpine.md", "Beta.md"] {
                std::fs::write(root.join(name), "").unwrap();
            }
            Self(root)
        }
    }

    impl Drop for TempVault {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn markdown_file_types() -> RegisteredFileTypes {
        RegisteredFileTypes::new([(
            "md".to_string(),
            crate::document::file_types::FileTypeCapabilities {
                text_search: true,
                wiki_links: true,
                yaml_frontmatter: true,
            },
        )])
    }

    fn catalog_for(root: &Path) -> VaultCatalog {
        let catalog = VaultCatalog::open(root, markdown_file_types()).unwrap();
        catalog.wait_until_ready(Duration::from_secs(5));
        catalog
    }

    #[test]
    fn detects_an_open_wiki_link_on_the_current_line() {
        let text = Rope::from("before [[Note");
        let link = wiki_link_at(&text, text.len()).unwrap();
        assert_eq!(link.query, "Note");
        assert_eq!(link.replace_start, "before ".len());
        assert_eq!(link.replace_end, text.len());
    }

    #[test]
    fn replaces_the_auto_inserted_closing_brackets() {
        let text = Rope::from("see [[");
        let offset = text.len();
        let text = Rope::from(format!("{text}]]"));
        let link = wiki_link_at(&text, offset).unwrap();
        assert_eq!(link.replace_start, "see ".len());
        assert_eq!(link.replace_end, offset.saturating_add(2));
    }

    #[test]
    fn ignores_closed_links_aliases_and_anchors() {
        for source in [
            "[[Note]]",
            "[[Note|label",
            "[[Note#heading",
            "text without a link",
        ] {
            let text = Rope::from(source);
            assert!(wiki_link_at(&text, text.len()).is_none(), "{source}");
        }
    }

    #[test]
    fn allows_folders_in_the_query() {
        let text = Rope::from("[[formats/Mar");
        let link = wiki_link_at(&text, text.len()).unwrap();
        assert_eq!(link.query, "formats/Mar");
    }

    #[test]
    fn highlights_the_matched_part_of_a_file_name() {
        assert_eq!(highlight_in("List of signal", "sign"), Some(8..12));
        assert_eq!(highlight_in("List of signal", "SIGN"), Some(8..12));
        assert_eq!(highlight_in("List of signal", "list"), Some(0..4));
        assert_eq!(highlight_in("List of signal", "zzz"), None);
        assert_eq!(highlight_in("List of signal", ""), None);
    }

    #[test]
    fn splits_folder_queries_for_highlighting() {
        assert_eq!(query_parts("formats/Mar"), (Some("formats"), "Mar"));
        assert_eq!(query_parts("a/nested/Note"), (Some("nested"), "Note"));
        assert_eq!(query_parts("plain"), (None, "plain"));
    }

    #[test]
    fn catalog_suggestions_filter_sort_and_qualify_duplicates() {
        let vault = TempVault::new();
        for relative in ["alpha/Note.md", "beta/nested/Note.md", "beta/Other.md"] {
            let path = vault.0.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "").unwrap();
        }
        let source = vault.0.join("Source.md");
        let catalog = catalog_for(&vault.0);

        let suggestions = catalog.suggest_wiki_links("note", Some(&source), MAX_SUGGESTIONS);
        let rows = suggestions
            .iter()
            .map(|suggestion| {
                (
                    suggestion.label.as_str(),
                    suggestion.detail.as_deref(),
                    suggestion.target.as_str(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rows,
            [
                ("Note", Some("alpha"), "alpha/Note"),
                ("Note", Some("beta/nested"), "beta/nested/Note"),
            ]
        );

        let suggestions = catalog.suggest_wiki_links("oth", Some(&source), MAX_SUGGESTIONS);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].label, "Other");
        assert_eq!(suggestions[0].detail.as_deref(), Some("beta"));
        assert_eq!(suggestions[0].target, "Other");

        let suggestions = catalog.suggest_wiki_links("alpha/", Some(&source), MAX_SUGGESTIONS);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].target, "alpha/Note");

        let suggestions = catalog.suggest_wiki_links("sour", Some(&source), MAX_SUGGESTIONS);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn catalog_suggestions_are_limited() {
        let vault = TempVault::new();
        for index in 0..MAX_SUGGESTIONS.saturating_add(8) {
            std::fs::write(vault.0.join(format!("Match{index:02}.md")), "").unwrap();
        }
        let catalog = catalog_for(&vault.0);

        let suggestions = catalog.suggest_wiki_links("match", None, MAX_SUGGESTIONS);
        assert_eq!(suggestions.len(), MAX_SUGGESTIONS);
    }
}
