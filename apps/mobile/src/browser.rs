use std::path::{Path, PathBuf};

use datalith_ui::fonts::ResolvedFonts;
use datalith_ui::touch::TouchSize as _;
use datalith_vault::{CatalogState, source};
use gpui_kit::component::button::{Button, ButtonGroup, ButtonVariants as _};
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Selectable as _, h_flex, input::Input, list::ListItem, v_flex,
};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, Context, HighlightStyle, StyledText, div, px};

use super::workspace::{Workspace, name};

pub struct Folder {
    pub path: PathBuf,
    files: Vec<(PathBuf, bool)>,
    scroll: gpui_kit::ScrollHandle,
}

impl Folder {
    pub fn new(path: PathBuf) -> std::io::Result<Self> {
        let mut folder = Self {
            path,
            files: Vec::new(),
            scroll: gpui_kit::ScrollHandle::new(),
        };
        folder.reload()?;
        Ok(folder)
    }

    pub fn reload(&mut self) -> std::io::Result<()> {
        let mut files = Vec::new();
        for path in source::read_dir(&self.path)? {
            if !name(&path).starts_with('.') && (source::is_read_only(&path) || !path.is_symlink())
            {
                files.push((path.clone(), source::is_dir(&path)));
            }
        }
        files.sort_by_cached_key(|(path, dir)| (!dir, name(path).to_lowercase()));
        self.files = files;
        Ok(())
    }

    pub(super) fn scroll_y(&self) -> f32 {
        self.scroll.offset().y.into()
    }
    pub(super) fn restore(&self, y: f32) {
        self.scroll.set_offset(gpui_kit::point(px(0.), px(y)));
    }

    /// One file or folder row; the path is its identity.
    fn row(
        workspace: &Workspace,
        path: &Path,
        folder: bool,
        detail: Option<AnyElement>,
        selected: Option<&Path>,
        cx: &Context<Workspace>,
    ) -> impl IntoElement + use<> {
        let label = if folder {
            name(path)
        } else {
            super::workspace::title(path)
        };
        let icon = if folder {
            Icon::new(IconName::Folder)
        } else {
            Icon::new(workspace.registry.config_for(Some(path)).icon)
        };
        let active = Some(path) == selected;
        let text = v_flex()
            .flex_1()
            .min_w_0()
            .py_1()
            .child(div().truncate().child(label.clone()))
            .children(detail);
        let target = path.to_owned();
        let menu = (!source::is_read_only(path)).then(|| Workspace::file_menu(path, folder, cx));
        let id = gpui_kit::SharedString::from(format!("entry:{}", path.display()));
        h_flex()
            .w_full()
            .child(
                ListItem::new(id)
                    .flex_1()
                    .min_w_0()
                    .accessibility_label(label)
                    .flex_shrink_0()
                    .min_h(px(48.))
                    .px_2()
                    .when(active, |row| row.bg(cx.theme().selection))
                    .child(
                        h_flex()
                            .w_full()
                            .gap_3()
                            .child(icon.size_4())
                            .child(text)
                            .when(folder, |row| {
                                row.child(Icon::new(IconName::ChevronRight).size_4())
                            }),
                    )
                    .on_click(cx.listener(move |v, _, w, cx| v.select(target.clone(), w, cx))),
            )
            .children(menu)
    }

    fn subtitle(text: String, cx: &Context<Workspace>) -> AnyElement {
        div()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .truncate()
            .child(text)
            .into_any_element()
    }

    fn section(title: &'static str, cx: &Context<Workspace>) -> impl IntoElement + use<> {
        div()
            .px_2()
            .pt_3()
            .pb_1()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(title)
    }

    fn notice(text: String, cx: &Context<Workspace>) -> impl IntoElement + use<> {
        div()
            .p_2()
            .text_color(cx.theme().muted_foreground)
            .whitespace_normal()
            .child(text)
    }

    fn rows(
        &self,
        workspace: &Workspace,
        selected: Option<&Path>,
        cx: &Context<Workspace>,
    ) -> impl IntoElement + use<> {
        let mut rows = v_flex()
            .id("files")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .p_2();
        let search = &workspace.search;
        let query = search.query(cx);
        let query = query.trim();
        if query.is_empty() {
            if search.focused && !search.recent.is_empty() {
                rows = rows.child(Self::section("Recent", cx));
                for path in &search.recent {
                    let parent = workspace.parent_label(path).map(|p| Self::subtitle(p, cx));
                    rows = rows.child(Self::row(workspace, path, false, parent, selected, cx));
                }
                return rows;
            }
            for (path, folder) in &self.files {
                rows = rows.child(Self::row(workspace, path, *folder, None, selected, cx));
            }
            if self.files.is_empty() {
                rows = rows.child(Self::notice("Empty folder".into(), cx));
            }
            return rows;
        }
        let results = &search.results;
        if !results.names.is_empty() {
            rows = rows.child(Self::section("Names", cx));
            for path in &results.names {
                let parent = workspace.parent_label(path).map(|p| Self::subtitle(p, cx));
                rows = rows.child(Self::row(workspace, path, false, parent, selected, cx));
            }
        }
        if !results.contents.is_empty() {
            rows = rows.child(Self::section("Contents", cx));
            let highlight = HighlightStyle {
                background_color: Some(cx.theme().warning.opacity(0.3)),
                font_weight: Some(gpui_kit::FontWeight::SEMIBOLD),
                ..Default::default()
            };
            for hit in &results.contents {
                let snippet = StyledText::new(hit.snippet.clone())
                    .with_highlights(hit.highlights.iter().cloned().map(|r| (r, highlight)));
                let detail = v_flex()
                    .children(
                        workspace
                            .parent_label(&hit.path)
                            .map(|p| Self::subtitle(p, cx)),
                    )
                    .child(
                        div()
                            .text_sm()
                            .whitespace_normal()
                            .line_clamp(2)
                            .child(snippet),
                    )
                    .into_any_element();
                rows = rows.child(Self::row(
                    workspace,
                    &hit.path,
                    false,
                    Some(detail),
                    selected,
                    cx,
                ));
            }
        }
        if results.names.is_empty() && results.contents.is_empty() {
            rows = rows.children(self.empty_results(workspace, query, cx));
        }
        rows
    }

    /// Progress, or what was searched and how to widen it.
    fn empty_results(
        &self,
        workspace: &Workspace,
        query: &str,
        cx: &Context<Workspace>,
    ) -> Vec<AnyElement> {
        let search = &workspace.search;
        let mut rows = Vec::new();
        let indexing = workspace
            .catalog
            .as_ref()
            .is_some_and(|c| c.state() == CatalogState::Syncing);
        if search.task.is_some() || indexing {
            rows.push(
                Self::notice(
                    if indexing {
                        "Indexing…"
                    } else {
                        "Searching…"
                    }
                    .into(),
                    cx,
                )
                .into_any_element(),
            );
        } else {
            let scope = if search.folder_only {
                workspace.folder_name(&self.path)
            } else {
                "the vault".into()
            };
            rows.push(
                Self::notice(format!("No results for “{query}” in {scope}"), cx).into_any_element(),
            );
            if search.folder_only {
                rows.push(
                    div()
                        .px_2()
                        .child(
                            Button::new("search-everywhere")
                                .outline()
                                .touch_h(44.)
                                .label("Search entire vault")
                                .on_click(cx.listener(|s, _, _, cx| s.set_search_scope(false, cx))),
                        )
                        .into_any_element(),
                );
            }
        }
        rows
    }

    /// Vault | ‹folder›: visible from a subfolder so the limit is a choice.
    fn scope(
        &self,
        workspace: &Workspace,
        cx: &Context<Workspace>,
    ) -> Option<impl IntoElement + use<>> {
        let folder_only = workspace.search.folder_only;
        // Only while searching: browsing a folder needs no scope control.
        if workspace.root.as_ref() == Some(&self.path)
            || (!folder_only && workspace.search.query(cx).trim().is_empty())
        {
            return None;
        }
        Some(
            div().px_3().pb_1().child(
                ButtonGroup::new("search-scope")
                    .outline()
                    .child(
                        Button::new("scope-vault")
                            .touch_h(36.)
                            .label("Vault")
                            .selected(!folder_only),
                    )
                    .child(
                        Button::new("scope-folder")
                            .touch_h(36.)
                            .label(workspace.folder_name(&self.path))
                            .selected(folder_only),
                    )
                    .on_click(cx.listener(|s, clicks: &Vec<usize>, _, cx| {
                        if let Some(ix) = clicks.first() {
                            s.set_search_scope(*ix == 1, cx);
                        }
                    })),
            ),
        )
    }

    /// Names the place: the vault or folder, with a lock for read-only
    /// vaults. Touching it lists the enclosing folders, which is the only way
    /// up the tree; Back always returns to the previous place.
    fn header(&self, workspace: &Workspace, cx: &Context<Workspace>) -> impl IntoElement + use<> {
        let mut ancestors: Vec<PathBuf> = workspace.root.as_ref().map_or_else(Vec::new, |root| {
            self.path
                .ancestors()
                .skip(1)
                .take_while(|p| p.starts_with(root))
                .map(Path::to_owned)
                .collect()
        });
        ancestors.reverse();
        let read_only = workspace.root.as_deref().is_some_and(source::is_read_only);
        let name = workspace.folder_name(&self.path);
        let title = h_flex()
            .w_full()
            .min_w_0()
            .gap_2()
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .font_family(cx.global::<ResolvedFonts>().headings.clone())
                    .text_xl()
                    .child(name.clone()),
            )
            .when(!ancestors.is_empty(), |el| {
                el.child(
                    Icon::new(IconName::ChevronDown)
                        .size_4()
                        .text_color(cx.theme().muted_foreground),
                )
            })
            .child(div().flex_1())
            .when(read_only, |el| {
                el.child(
                    h_flex()
                        .gap_1()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(Icon::new(datalith_ui::icons::DatalithIcon::Lock).size_4())
                        .child("Read only"),
                )
            });
        if ancestors.is_empty() {
            return div().px_4().py_2().child(title).into_any_element();
        }
        div()
            .px_2()
            .child(
                Button::new("folder-header")
                    .ghost()
                    .w_full()
                    .min_h(px(48.))
                    .accessibility_label(format!("{name}, show enclosing folders"))
                    .child(title)
                    .on_click(cx.listener(move |s, _, w, cx| {
                        s.choose(super::workspace::Choice::Ancestor(ancestors.clone()), w, cx);
                    })),
            )
            .into_any_element()
    }

    pub fn render(
        &self,
        workspace: &Workspace,
        selected: Option<&Path>,
        cx: &Context<Workspace>,
    ) -> impl IntoElement {
        let input = &workspace.search.input;
        let search = div().px_3().py_1().child(Input::new(input).cleanable(true));
        #[cfg(target_os = "ios")]
        let search = search.child(super::ios_input::registration(input));
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.header(workspace, cx))
            .child(search)
            .children(self.scope(workspace, cx))
            .child(self.rows(workspace, selected, cx))
    }
}
