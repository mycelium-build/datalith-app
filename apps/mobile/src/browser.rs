use std::path::{Path, PathBuf};

use datalith_ui::{fonts::ResolvedFonts, registry::FileRegistry};
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
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("Search files…"));
        let change = cx.subscribe(&query, |_, _, event, cx| {
            if matches!(event, gpui_kit::component::input::InputEvent::Change) {
                cx.notify();
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
        for entry in std::fs::read_dir(&self.path)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if !entry.file_name().to_string_lossy().starts_with('.')
                && (kind.is_dir() || kind.is_file())
            {
                files.push((entry.path(), kind.is_dir()));
            }
        }
        files.sort_by_cached_key(|(path, dir)| (!dir, super::workspace::name(path).to_lowercase()));
        self.files = files;
        Ok(())
    }

    pub fn render(
        &self,
        registry: &FileRegistry,
        selected: Option<&Path>,
        cx: &Context<Workspace>,
    ) -> impl IntoElement {
        let query = self.query.read(cx).value().to_lowercase();
        let mut rows = v_flex()
            .id("files")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .p_2();
        let mut has_matches = false;
        for (index, (path, folder)) in self.files.iter().enumerate() {
            let name = super::workspace::name(path);
            if !name.to_lowercase().contains(&query) {
                continue;
            }
            has_matches = true;
            let icon = if *folder {
                Icon::new(IconName::Folder)
            } else {
                Icon::new(registry.config_for(Some(path)).icon)
            };
            let active = Some(path.as_path()) == selected;
            let path = path.clone();
            rows = rows.child(
                ListItem::new(("entry", index))
                    .accessibility_label(name.clone())
                    .flex_shrink_0()
                    .h(px(48.))
                    .px_2()
                    .when(active, |row| row.bg(cx.theme().selection))
                    .child(
                        h_flex()
                            .w_full()
                            .gap_3()
                            .child(icon.size_4())
                            .child(div().flex_1().truncate().child(name))
                            .when(*folder, |row| {
                                row.child(Icon::new(IconName::ChevronRight).size_4())
                            }),
                    )
                    .on_click(cx.listener(move |v, _, w, cx| v.select(path.clone(), w, cx))),
            );
        }
        if !has_matches {
            rows = rows.child(div().p_2().text_color(cx.theme().muted_foreground).child(
                if query.is_empty() {
                    "Empty folder"
                } else {
                    "No matching files"
                },
            ));
        }
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .px_4()
                    .py_2()
                    .font_family(cx.global::<ResolvedFonts>().headings.clone())
                    .text_xl()
                    .child(super::workspace::name(&self.path)),
            )
            .child(
                div()
                    .px_3()
                    .py_1()
                    .child(Input::new(&self.query).cleanable(true)),
            )
            .child(rows)
    }
}
