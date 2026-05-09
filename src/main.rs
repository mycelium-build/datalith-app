use gpui::*;
use gpui_component::{
    h_flex,
    input::{Input, InputEvent, InputState},
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
    current_file: Option<PathBuf>,
    editor_state: Option<Entity<InputState>>,
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

    fn open_file(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if ext != "txt" && ext != "md" {
            return;
        }

        let content = fs::read_to_string(&path).unwrap_or_default();
        self.current_file = Some(path);
        self.editor_state = Some(cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .default_value(content)
        }));

        let _subscription = cx.subscribe_in(
            self.editor_state.as_ref().unwrap(),
            window,
            move |_view, editor, event, _window, _cx| {
                if let InputEvent::Change = event {
                    let content = editor.read(_cx).value();
                    if let Some(ref path) = _view.current_file {
                        let _ = fs::write(path, content.to_string());
                    }
                }
            },
        );

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
                                        let is_folder = entry.is_folder();
                                        let is_expanded = entry.is_expanded();
                                        let item_id = entry.item().id.clone();
                                        let item_label = entry.item().label.clone();
                                        let depth = entry.depth();

                                        view.update(cx, move |_this, cx| {
                                            let icon = if !is_folder {
                                                IconName::File
                                            } else if is_expanded {
                                                IconName::FolderOpen
                                            } else {
                                                IconName::Folder
                                            };

                                            ListItem::new(ix)
                                                .selected(selected)
                                                .pl(px(16.) * depth + px(12.))
                                                .child(
                                                    h_flex()
                                                        .gap_2()
                                                        .items_center()
                                                        .child(Icon::new(icon).size_4())
                                                        .child(item_label.clone()),
                                                )
                                                .on_click(cx.listener({
                                                    let item_id = item_id.clone();
                                                    let is_folder = is_folder;
                                                    move |this, _, window, cx| {
                                                        if !is_folder {
                                                            let path = PathBuf::from(item_id.to_string());
                                                            this.open_file(path, window, cx);
                                                        }
                                                    }
                                                }))
                                        })
                                    }
                                },
                            )),
                    ),
            )
            .child(match self.editor_state.as_ref() {
                Some(editor) => Input::new(editor).h_full().into_any_element(),
                None => div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(match self.root_path {
                        Some(_) => "Select a file from the sidebar",
                        None => "Select a folder from the menu bar",
                    })
                    .into_any_element(),
            })
    }
}

fn build_file_items(path: &Path) -> Vec<TreeItem> {
    let mut dirs = Vec::new();
    let mut files = Vec::new();
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
                dirs.push((name.clone(), TreeItem::new(path.to_string_lossy().to_string(), name).children(children)));
            } else {
                files.push((name.clone(), TreeItem::new(path.to_string_lossy().to_string(), name)));
            }
        }
    }
    dirs.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    files.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    let mut items: Vec<TreeItem> = dirs.into_iter().map(|(_, item)| item).collect();
    items.extend(files.into_iter().map(|(_, item)| item));
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
                    current_file: None,
                    editor_state: None,
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
