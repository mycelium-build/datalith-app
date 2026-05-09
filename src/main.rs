use gpui::*;
use gpui_component::{
    h_flex,
    list::ListItem,
    sidebar::SidebarHeader,
    tree::{self, TreeItem, TreeState},
    ActiveTheme, Icon, IconName, Root,
};

use std::fs;
use std::path::{Path, PathBuf};

actions!(datalith, [OpenCodex]);

struct AppState {
    view: Option<Entity<DatalithView>>,
}

impl Global for AppState {}

pub struct DatalithView {
    tree_state: Entity<TreeState>,
    root_path: Option<PathBuf>,
    root_name: SharedString,
}

impl DatalithView {
    fn set_root_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.root_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .into();
        self.root_path = Some(path.clone());
        let items = build_file_items(&path);
        self.tree_state.update(cx, |state, cx| {
            state.set_items(items, cx);
        });
        cx.notify();
    }
}

impl Render for DatalithView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .size_full()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w(px(260.))
                    .h_full()
                    .bg(cx.theme().sidebar)
                    .border_r_1()
                    .border_color(cx.theme().border)
                    .child(
                        SidebarHeader::new()
                            .p_2()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(Icon::new(IconName::Folder))
                                    .child(self.root_name.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(tree::tree(
                                &self.tree_state,
                                {
                                    let view = cx.entity();
                                    move |ix, entry, selected, _window, cx| {
                                        view.update(cx, |_this, _cx| {
                                            let item = entry.item();
                                            let icon = if !entry.is_folder() {
                                                IconName::File
                                            } else if entry.is_expanded() {
                                                IconName::FolderOpen
                                            } else {
                                                IconName::Folder
                                            };

                                            ListItem::new(ix)
                                                .selected(selected)
                                                .pl(px(16.) * entry.depth() + px(12.))
                                                .child(
                                                    h_flex()
                                                        .gap_2()
                                                        .items_center()
                                                        .child(Icon::new(icon).size_4())
                                                        .child(item.label.clone()),
                                                )
                                        })
                                    }
                                },
                            )),
                    ),
            )
            .child(
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child("Select a folder from the menu bar"),
            )
    }
}

fn build_file_items(path: &Path) -> Vec<TreeItem> {
    let mut items = Vec::new();
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown")
                .to_string();

            if path.is_dir() {
                let children = build_file_items(&path);
                items.push(
                    TreeItem::new(path.to_string_lossy().to_string(), name).children(children),
                );
            } else {
                items.push(TreeItem::new(path.to_string_lossy().to_string(), name));
            }
        }
    }
    items
}

fn main() {
    let app = gpui_platform::application().with_assets(gpui_component_assets::Assets);

    app.run(move |cx| {
        gpui_component::init(cx);

        cx.set_global(AppState { view: None });
        cx.on_action(open_codex);
        cx.set_menus([Menu::new("datalith").items([
            MenuItem::action("Open codex", OpenCodex),
        ])]);

        cx.spawn(async move |cx| {
            cx.open_window(WindowOptions::default(), |window, cx| {
                let view = cx.new(|cx| DatalithView {
                    tree_state: cx.new(|cx| TreeState::new(cx)),
                    root_path: None,
                    root_name: "No folder open".into(),
                });
                cx.update_global(|state: &mut AppState, _| {
                    state.view = Some(view.clone());
                });
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("Failed to open window");
        })
        .detach();
    });
}

fn open_codex(_: &OpenCodex, cx: &mut App) {
    let rx = cx.prompt_for_paths(PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some("Select a folder".into()),
    });
    cx.spawn(async move |cx| {
        if let Ok(Ok(Some(paths))) = rx.await {
            if let Some(path) = paths.into_iter().next() {
                let view_opt = cx.read_global(|state: &AppState, _| state.view.clone());
                if let Some(view) = view_opt {
                    cx.update_entity(&view, |view, cx| {
                        view.set_root_path(path, cx);
                    });
                }
            }
        }
    })
    .detach();
}
