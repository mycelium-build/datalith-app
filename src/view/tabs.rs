use std::fs;
use std::path::PathBuf;

use gpui::*;
use gpui_component::input::{InputEvent, InputState};

use super::{DatalithView, OpenFile};
use crate::utils::is_supported_file;

impl DatalithView {
    pub fn open_file(
        &mut self,
        path: PathBuf,
        new_tab: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !is_supported_file(&path) {
            return;
        }

        if let Some(index) = self.open_files.iter().position(|f| f.path == path) {
            self.active_tab = index;
            if let Some(ref state) = self.open_files[index].state {
                state.focus_handle(cx).focus(window, cx);
            }
            cx.notify();
            return;
        }

        let content = fs::read_to_string(&path).unwrap_or_default();
        let state = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .searchable(true)
                .default_value(content)
        });

        let sub = {
            let path = path.clone();
            cx.subscribe_in(&state, window, move |_view, editor, event, _window, _cx| {
                if let InputEvent::Change = event {
                    let content = editor.read(_cx).value();
                    let _ = fs::write(&path, content.to_string());
                }
            })
        };

        if new_tab || self.open_files.is_empty() {
            state.focus_handle(cx).focus(window, cx);
            self.open_files.push(OpenFile {
                path,
                state: Some(state),
                _sub: Some(sub),
            });
            self.active_tab = self.open_files.len() - 1;
        } else {
            let active = self.active_tab.min(self.open_files.len() - 1);
            let old_path = self.open_files[active].path.clone();
            if !old_path.as_os_str().is_empty() {
                self.track_file_edited(&old_path);
            }
            self.open_files[active] = OpenFile {
                path,
                state: Some(state),
                _sub: Some(sub),
            };
            if let Some(ref s) = self.open_files[active].state {
                s.focus_handle(cx).focus(window, cx);
            }
        }
        cx.notify();
    }

    pub fn new_empty_tab(&mut self, cx: &mut Context<Self>) {
        self.open_files.push(OpenFile {
            path: PathBuf::new(),
            state: None,
            _sub: None,
        });
        self.active_tab = self.open_files.len() - 1;
        cx.notify();
    }

    pub fn close_active_tab(&mut self, cx: &mut Context<Self>) {
        if self.open_files.is_empty() {
            return;
        }
        let index = self.active_tab.min(self.open_files.len().saturating_sub(1));
        self.close_tab(index, cx);
    }

    pub fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
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
}
