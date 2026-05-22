use std::fs;
use std::path::{Path, PathBuf};

use gpui::*;
use gpui_component::input::{InputEvent, InputState};
use percent_encoding::percent_decode_str;

use super::markdown_editor::{MarkdownEditor, MarkdownEditorEvent};
use super::{DatalithView, OpenFile};
use crate::utils::is_supported_file;

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .map(|ext| ext.to_string_lossy().to_lowercase() == "md")
        .unwrap_or(false)
}

impl DatalithView {
    pub(crate) fn open_file(
        &mut self,
        path: PathBuf,
        new_tab: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !is_supported_file(&path) {
            return;
        }

        if !new_tab && !self.open_files.is_empty() {
            self.push_navigation_state(&path);
        }

        if !self.in_navigation {
            if let Some(index) = self.open_files.iter().position(|f| f.path == path) {
                self.active_tab = index;
                if let Some(ref state) = self.open_files[index].state {
                    state.focus_handle(cx).focus(window, cx);
                }
                cx.notify();
                return;
            }
        }

        let content = fs::read_to_string(&path).unwrap_or_default();
        let state = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .searchable(true)
                .default_value(content.clone())
        });

        let markdown_editor = if is_markdown(&path) {
            Some(cx.new(|cx| MarkdownEditor::new(state.clone(), true, cx)))
        } else {
            None
        };

        let md_sub = if let Some(ref editor) = markdown_editor {
            Some(cx.subscribe_in(
                editor,
                window,
                move |view, _, event, window, cx| match event {
                    MarkdownEditorEvent::LinkClicked(url, new_tab) => {
                        let decoded_url = percent_decode_str(url).decode_utf8_lossy();
                        if let Some(ref cache) = view.link_cache {
                            if let Some(resolved) = cache.resolve(&decoded_url) {
                                view.open_file(resolved, *new_tab, window, cx);
                                return;
                            }
                        }
                        cx.open_url(url);
                    }
                },
            ))
        } else {
            None
        };

        let sub = {
            let path = path.clone();
            cx.subscribe_in(&state, window, move |_view, editor, event, _window, _cx| {
                if let InputEvent::Change = event {
                    let content = editor.read(_cx).value();
                    let _ = fs::write(&path, content.to_string());
                }
            })
        };

        let (nav_stack, nav_pos) = if new_tab || self.open_files.is_empty() {
            (vec![path.clone()], 0)
        } else {
            let active = self.active_tab.min(self.open_files.len() - 1);
            let old = &self.open_files[active];
            (old.navigation_stack.clone(), old.navigation_position)
        };

        let open_file = OpenFile {
            path: path.clone(),
            state: Some(state.clone()),
            markdown_editor,
            _sub: Some(sub),
            _md_sub: md_sub,
            editor_mode: true,
            navigation_stack: nav_stack,
            navigation_position: nav_pos,
        };

        if new_tab || self.open_files.is_empty() {
            state.focus_handle(cx).focus(window, cx);
            self.open_files.push(open_file);
            self.active_tab = self.open_files.len() - 1;
        } else {
            let active = self.active_tab.min(self.open_files.len() - 1);
            let old_path = self.open_files[active].path.clone();
            if !old_path.as_os_str().is_empty() {
                self.track_file_edited(&old_path);
            }
            self.open_files[active] = open_file;
            if let Some(ref s) = self.open_files[active].state {
                s.focus_handle(cx).focus(window, cx);
            }
        }
        cx.notify();
    }

    pub(crate) fn new_empty_tab(&mut self, cx: &mut Context<Self>) {
        self.open_files.push(OpenFile {
            path: PathBuf::new(),
            state: None,
            markdown_editor: None,
            _sub: None,
            _md_sub: None,
            editor_mode: true,
            navigation_stack: Vec::new(),
            navigation_position: 0,
        });
        self.active_tab = self.open_files.len() - 1;
        cx.notify();
    }

    pub(crate) fn close_active_tab(&mut self, cx: &mut Context<Self>) {
        if self.open_files.is_empty() {
            return;
        }
        let index = self.active_tab.min(self.open_files.len().saturating_sub(1));
        self.close_tab(index, cx);
    }

    pub(crate) fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.open_files.len() {
            return;
        }
        let path = self.open_files[index].path.clone();
        if !path.as_os_str().is_empty() {
            self.track_file_edited(&path);
        }
        self.open_files.remove(index);
        if self.open_files.is_empty() {
            self.active_tab = 0;
        } else if self.active_tab > index && self.active_tab > 0 {
            self.active_tab -= 1;
        } else if self.active_tab >= self.open_files.len() {
            self.active_tab = self.open_files.len() - 1;
        }
        cx.notify();
    }

    pub(crate) fn close_tab_for_file(&mut self, path: &PathBuf, cx: &mut Context<Self>) {
        let indices: Vec<usize> = self
            .open_files
            .iter()
            .enumerate()
            .filter(|(_, f)| f.path == *path)
            .map(|(i, _)| i)
            .collect();
        for index in indices.into_iter().rev() {
            self.close_tab(index, cx);
        }
    }

    pub(crate) fn close_tabs_under(&mut self, root: &Path, cx: &mut Context<Self>) {
        let prefix = root.to_string_lossy().to_string();
        let indices: Vec<usize> = self
            .open_files
            .iter()
            .enumerate()
            .filter(|(_, f)| f.path.to_string_lossy().starts_with(&prefix))
            .map(|(i, _)| i)
            .collect();
        for index in indices.into_iter().rev() {
            self.close_tab(index, cx);
        }
    }

    pub(crate) fn push_navigation_state(&mut self, to_path: &PathBuf) {
        if self.in_navigation {
            return;
        }
        let active = self.active_tab.min(self.open_files.len().saturating_sub(1));
        let file = &mut self.open_files[active];

        if file.navigation_stack.is_empty() {
            file.navigation_stack.push(to_path.clone());
            file.navigation_position = 0;
            return;
        }

        if file.navigation_position < file.navigation_stack.len()
            && file.navigation_stack[file.navigation_position] == *to_path
        {
            return;
        }

        if file.navigation_position + 1 < file.navigation_stack.len() {
            file.navigation_stack.truncate(file.navigation_position + 1);
        }

        file.navigation_stack.push(to_path.clone());
        file.navigation_position = file.navigation_stack.len() - 1;
    }

    pub(crate) fn go_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open_files.is_empty() {
            return;
        }
        let active = self.active_tab.min(self.open_files.len().saturating_sub(1));
        let file = &self.open_files[active];
        if file.navigation_position == 0 {
            return;
        }
        let target_pos = file.navigation_position - 1;
        let path = file.navigation_stack[target_pos].clone();

        self.in_navigation = true;
        self.open_file(path, false, window, cx);
        self.in_navigation = false;

        let active = self.active_tab.min(self.open_files.len().saturating_sub(1));
        self.open_files[active].navigation_position = target_pos;
    }

    pub(crate) fn go_forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open_files.is_empty() {
            return;
        }
        let active = self.active_tab.min(self.open_files.len().saturating_sub(1));
        let file = &self.open_files[active];
        if file.navigation_position + 1 >= file.navigation_stack.len() {
            return;
        }
        let target_pos = file.navigation_position + 1;
        let path = file.navigation_stack[target_pos].clone();

        self.in_navigation = true;
        self.open_file(path, false, window, cx);
        self.in_navigation = false;

        let active = self.active_tab.min(self.open_files.len().saturating_sub(1));
        self.open_files[active].navigation_position = target_pos;
    }

    pub(crate) fn can_go_back(&self) -> bool {
        if self.open_files.is_empty() {
            return false;
        }
        let active = self.active_tab.min(self.open_files.len().saturating_sub(1));
        self.open_files[active].navigation_position > 0
    }

    pub(crate) fn can_go_forward(&self) -> bool {
        if self.open_files.is_empty() {
            return false;
        }
        let active = self.active_tab.min(self.open_files.len().saturating_sub(1));
        self.open_files[active].navigation_position + 1
            < self.open_files[active].navigation_stack.len()
    }
}
