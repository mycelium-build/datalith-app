use std::path::{Path, PathBuf};

use datalith_ui::fonts::ResolvedFonts;
use datalith_vault::source;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, h_flex,
    input::{Input, InputState},
    list::ListItem,
    v_flex,
};
use gpui_kit::prelude::*;
use gpui_kit::{Context, Entity, Window, div, px};

use super::workspace::Workspace;

pub struct Folder {
    pub path: PathBuf,
    files: Vec<(PathBuf, bool)>,
    query: Entity<InputState>,
    _change: gpui_kit::Subscription,
    scroll: gpui_kit::ScrollHandle,
}

impl Folder {
    pub fn new(
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) -> std::io::Result<Self> {
        let placeholder = "Search…";
        let query = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let change = cx.subscribe(&query, |view: &mut Workspace, _, event, cx| {
            if matches!(event, gpui_kit::component::input::InputEvent::Change) {
                view.search_changed(cx);
            }
        });
        let mut folder = Self {
            path,
            files: Vec::new(),
            query,
            _change: change,
            scroll: gpui_kit::ScrollHandle::new(),
        };
        folder.reload()?;
        Ok(folder)
    }

    pub fn reload(&mut self) -> std::io::Result<()> {
        let mut files = Vec::new();
        for path in source::read_dir(&self.path)? {
            if !super::workspace::name(&path).starts_with('.')
                && (source::is_read_only(&path) || !path.is_symlink())
            {
                files.push((path.clone(), source::is_dir(&path)));
            }
        }
        files.sort_by_cached_key(|(path, dir)| (!dir, super::workspace::name(path).to_lowercase()));
        self.files = files;
        Ok(())
    }

    pub(super) fn scroll_y(&self) -> f32 {
        self.scroll.offset().y.into()
    }
    pub(super) fn restore(&self, query: &str, y: f32, w: &mut Window, cx: &mut Context<Workspace>) {
        if self.query(cx) != query {
            self.query.update(cx, |i, cx| i.set_value(query, w, cx));
        }
        self.scroll.set_offset(gpui_kit::point(px(0.), px(y)));
    }
    pub(super) fn query(&self, cx: &gpui_kit::App) -> String {
        self.query.read(cx).value().to_string()
    }

    fn file_rows(
        &self,
        workspace: &Workspace,
        query: &str,
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
        let mut has_matches = false;
        let results: Vec<_>;
        let global = workspace.whole_vault && !query.is_empty();
        let entries = if global {
            results = workspace
                .search_results
                .iter()
                .map(|p| (p.clone(), false))
                .collect();
            &results
        } else {
            &self.files
        };
        for (path, folder) in entries {
            let name = super::workspace::name(path);
            if !global && !name.to_lowercase().contains(query) {
                continue;
            }
            has_matches = true;
            let icon = if *folder {
                Icon::new(IconName::Folder)
            } else {
                Icon::new(workspace.registry.config_for(Some(path)).icon)
            };
            let active = Some(path.as_path()) == selected;
            let parent = global
                .then(|| {
                    workspace
                        .root
                        .as_ref()
                        .and_then(|r| path.strip_prefix(r).ok())
                        .and_then(Path::parent)
                })
                .flatten()
                .filter(|dir| !dir.as_os_str().is_empty())
                .map(|dir| dir.display().to_string());
            let label = v_flex()
                .flex_1()
                .min_w_0()
                .child(div().truncate().child(name.clone()))
                .children(parent.map(|p| {
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .truncate()
                        .child(p)
                }));
            let path = path.clone();
            let menu =
                (!source::is_read_only(&path)).then(|| Workspace::file_menu(&path, *folder, cx));
            let id = gpui_kit::SharedString::from(format!("entry:{}", path.display()));
            rows = rows.child(
                h_flex()
                    .w_full()
                    .child(
                        ListItem::new(id)
                            .flex_1()
                            .min_w_0()
                            .accessibility_label(name)
                            .flex_shrink_0()
                            .h(px(48.))
                            .px_2()
                            .when(active, |row| row.bg(cx.theme().selection))
                            .child(
                                h_flex()
                                    .w_full()
                                    .gap_3()
                                    .child(icon.size_4())
                                    .child(label)
                                    .when(*folder, |row| {
                                        row.child(Icon::new(IconName::ChevronRight).size_4())
                                    }),
                            )
                            .on_click(
                                cx.listener(move |v, _, w, cx| v.select(path.clone(), w, cx)),
                            ),
                    )
                    .children(menu),
            );
        }
        if !has_matches {
            rows = rows.child(div().p_2().text_color(cx.theme().muted_foreground).child(
                if global && workspace.is_searching() {
                    "Searching…"
                } else if query.is_empty() {
                    "Empty folder"
                } else {
                    "No matching files"
                },
            ));
        }
        rows
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
        let search = div()
            .px_3()
            .py_1()
            .child(Input::new(&self.query).cleanable(true));
        #[cfg(target_os = "ios")]
        let search = search.child(super::ios_input::registration(&self.query));
        let query = self.query.read(cx).value().to_lowercase();
        let rows = self.file_rows(workspace, &query, selected, cx);
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.header(workspace, cx))
            .child(search)
            .when(!query.is_empty() || workspace.whole_vault, |el| {
                el.child(
                    h_flex().px_2().child(
                        Button::new("search-scope")
                            .ghost()
                            .h(px(44.))
                            .label(if workspace.whole_vault {
                                "Entire vault"
                            } else {
                                "This folder"
                            })
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.whole_vault = !s.whole_vault;
                                s.search_changed(cx);
                            })),
                    ),
                )
            })
            .child(rows)
    }
}
