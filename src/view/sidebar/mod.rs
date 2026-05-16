pub(crate) mod file_tree;
mod header;
mod navigation;

use std::path::PathBuf;

use gpui::*;
use gpui_component::{
    ActiveTheme,
    input::{InputEvent, InputState},
    menu::ContextMenuExt,
    select::Select,
};

use crate::actions::{CopyPath, Delete, Duplicate, NewFile, NewFolder, OpenInExplorer, Rename};
use crate::consts::{BORDER_WIDTH, SIDEBAR_WIDTH};
use crate::fs_ops;
use crate::utils::file_name_str;

use super::DatalithView;

#[derive(Clone)]
pub(crate) struct DragFile {
    pub(crate) path: PathBuf,
}

impl Render for DragFile {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let name = file_name_str(&self.path).to_string();
        div()
            .px_2()
            .py_1()
            .bg(cx.theme().accent)
            .text_color(cx.theme().accent_foreground)
            .rounded_sm()
            .text_sm()
            .child(name)
    }
}

impl DatalithView {
    pub(crate) fn rename_file(
        &mut self,
        old_path: &std::path::Path,
        new_path: &std::path::Path,
        cx: &mut Context<Self>,
    ) {
        if old_path == new_path {
            return;
        }
        let _ = std::fs::rename(old_path, new_path);
        self.track_file_rename(old_path, new_path);
        for f in &mut self.open_files {
            if f.path == old_path {
                f.path = new_path.to_path_buf();
            }
        }
        if self.pending_open.as_deref() == Some(old_path) {
            self.pending_open = Some(new_path.to_path_buf());
        }
        self.refresh_tree(cx);
    }

    fn handle_file_move(&mut self, old_path: PathBuf, new_path: PathBuf, cx: &mut Context<Self>) {
        self.rename_file(&old_path, &new_path, cx);
    }

    pub(crate) fn commit_rename(&mut self, cx: &mut Context<Self>) {
        if let (Some(ref rename_state), Some(ref target)) =
            (self.rename_state.clone(), self.rename_target.clone())
        {
            let new_name = rename_state.read(cx).value().to_string();
            if !new_name.is_empty() {
                let old_ext = if !target.is_dir() {
                    target
                        .file_name()
                        .and_then(|n| n.to_str())
                        .and_then(|name| {
                            name.rfind('.')
                                .and_then(|dot| if dot > 0 { Some(&name[dot..]) } else { None })
                                .map(|e| e.to_string())
                        })
                } else {
                    None
                };
                let mut name = new_name;
                if let Some(ref ext) = old_ext && !name.contains('.') {
                    name.push_str(ext);
                }
                if let Some(parent) = target.parent() {
                    let candidate = parent.join(&name);
                    if candidate != *target {
                        let final_path = fs_ops::unique_name(parent, &name);
                        self.rename_file(target, &final_path, cx);
                    }
                }
            }
        }
        self.clear_rename_state();
    }

    fn clear_rename_state(&mut self) {
        self.rename_target = None;
        self.rename_state = None;
        self._rename_sub = None;
    }

    fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        self.clear_rename_state();
        self.refresh_tree(cx);
    }

    fn ensure_rename_state(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ref target) = self.rename_target.clone() else {
            self.rename_state = None;
            return;
        };
        if self.rename_state.is_some() {
            return;
        }

        let current = file_name_str(target).to_string();
        let state = cx.new(|cx| InputState::new(window, cx).default_value(current.as_str()));

        self._rename_sub = Some(cx.subscribe_in(
            &state,
            window,
            move |this, _input, event, _window, cx| match event {
                InputEvent::PressEnter { .. } | InputEvent::Blur => {
                    this.commit_rename(cx);
                }
                _ => {}
            },
        ));

        state.focus_handle(cx).focus(window, cx);
        self.rename_state = Some(state.clone());
        window.dispatch_action(Box::new(gpui_component::input::SelectAll), cx);
    }

    fn path_from_id(id: &SharedString) -> PathBuf {
        PathBuf::from(id.to_string())
    }

    pub(crate) fn render_sidebar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let tree_state = self.tree_state.clone();
        let view = cx.entity().clone();

        self.ensure_rename_state(window, cx);

        if self.pending_vault_refresh {
            self.refresh_vault_select(window, cx);
        }

        if let Some(ref root) = self.root_path {
            let root_str: SharedString = root.to_string_lossy().to_string().into();
            self.vault_select_state.update(cx, |state, cx| {
                state.set_selected_value(&root_str, window, cx);
            });
        }

        div()
            .flex()
            .flex_col()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .bg(cx.theme().tab_bar)
            .border_r(px(BORDER_WIDTH))
            .border_color(cx.theme().border)
            .track_focus(&self.sidebar_focus_handle)
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _event: &MouseDownEvent, window, cx| {
                    this.sidebar_focus_handle.focus(window, cx);
                    cx.stop_propagation();
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.on_sidebar_key_down(event, window, cx);
            }))
            .on_drop(cx.listener(move |this, drag: &DragFile, _window, cx| {
                if let (Some(root), Some(name)) = (&this.root_path, drag.path.file_name()) {
                    let new_path = root.join(name);
                    this.handle_file_move(drag.path.clone(), new_path, cx);
                }
            }))
            .child(self.render_sidebar_header(cx))
            .child(div().flex_1().child(self.render_file_tree(cx, &self.tree_state)))
            .child(
                div()
                    .border_t(px(BORDER_WIDTH))
                    .border_color(cx.theme().border)
                    .p_2()
                    .child(Select::new(&self.vault_select_state)),
            )
            .context_menu({
                let view = view.clone();
                let tree_state = tree_state.clone();
                move |menu, _window, cx| {
                    if let Some(entry) = tree_state.read(cx).right_clicked_entry() {
                        let path = Self::path_from_id(&entry.item().id);
                        view.update(cx, |v, _| v.context_menu_target = Some(path));
                        menu.menu("New File", Box::new(NewFile))
                            .menu("New Folder", Box::new(NewFolder))
                            .separator()
                            .menu("Rename", Box::new(Rename))
                            .menu("Delete", Box::new(Delete))
                            .menu("Duplicate", Box::new(Duplicate))
                            .separator()
                            .menu("Open in Explorer", Box::new(OpenInExplorer))
                            .menu("Copy Path", Box::new(CopyPath))
                    } else {
                        view.update(cx, |v, _| v.context_menu_target = v.root_path.clone());
                        menu.menu("New File", Box::new(NewFile))
                            .menu("New Folder", Box::new(NewFolder))
                            .separator()
                            .menu("Open in Explorer", Box::new(OpenInExplorer))
                            .menu("Copy Path", Box::new(CopyPath))
                    }
                }
            })
    }
}
