use std::fs;
use std::path::{Path, PathBuf};

use gpui::*;
use gpui_component::{
    Disableable, Icon, IconName, Sizable,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::InputEvent,
    tab::{Tab, TabBar},
};
use percent_encoding::percent_decode_str;

use super::file_handler::{FileHandler, FileHandlerEvent, ViewMode};
use super::{DatalithView, NavigationAction, OpenFile};
use crate::assets::PEN_ICON;
use crate::utils::file_name_str;

impl DatalithView {
    pub(crate) fn open_file(
        &mut self,
        path: PathBuf,
        new_tab: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.registry.is_supported(&path) {
            return;
        }

        if !new_tab && !self.open_files.is_empty() {
            self.push_navigation_state(&path);
        }

        if !self.in_navigation {
            if let Some(index) = self.open_files.iter().position(|f| f.path == path) {
                self.active_tab = index;
                self.open_files[index]
                    .handler
                    .read(cx)
                    .focus_handle(cx)
                    .focus(window, cx);
                cx.notify();
                return;
            }
        }

        let handler = cx.new(|cx| self.registry.create_handler(&path, window, cx));

        let sub = handler.read(cx).input().cloned().map(|state| {
            let path = path.clone();
            cx.subscribe_in(&state, window, move |_view, state, event, _window, cx| {
                if let InputEvent::Change = event {
                    let content = state.read(cx).value();
                    let _ = fs::write(&path, content.to_string());
                }
            })
        });

        let event_sub = cx.subscribe_in(
            &handler,
            window,
            |view, _handler, event: &FileHandlerEvent, window, cx| match event {
                FileHandlerEvent::LinkClicked(url, new_tab) => {
                    let decoded_url = percent_decode_str(url).decode_utf8_lossy();
                    if let Some(ref cache) = view.link_cache {
                        if let Some(resolved) = cache.resolve(&decoded_url) {
                            view.open_file(resolved.clone(), *new_tab, window, cx);
                            return;
                        }
                    }
                    cx.open_url(url);
                }
            },
        );

        let (nav_stack, nav_pos) = if new_tab || self.open_files.is_empty() {
            (vec![path.clone()], 0)
        } else {
            let active = self.active_tab.min(self.open_files.len() - 1);
            let old = &self.open_files[active];
            (old.navigation_stack.clone(), old.navigation_position)
        };

        let open_file = OpenFile {
            path: path.clone(),
            handler,
            _sub: sub,
            _event_sub: Some(event_sub),
            navigation_stack: nav_stack,
            navigation_position: nav_pos,
        };

        if new_tab || self.open_files.is_empty() {
            self.open_files.push(open_file);
            self.active_tab = self.open_files.len() - 1;
            self.open_files[self.active_tab]
                .handler
                .read(cx)
                .focus_handle(cx)
                .focus(window, cx);
        } else {
            let active = self.active_tab.min(self.open_files.len() - 1);
            let old_path = self.open_files[active].path.clone();
            if !old_path.as_os_str().is_empty() {
                self.track_file_edited(&old_path);
            }
            self.open_files[active] = open_file;
            self.open_files[active]
                .handler
                .read(cx)
                .focus_handle(cx)
                .focus(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn new_empty_tab(&mut self, cx: &mut Context<Self>) {
        let handler = cx.new(|_cx| FileHandler::new(ViewMode::Edit, None, None));
        self.open_files.push(OpenFile {
            path: PathBuf::new(),
            handler,
            _sub: None,
            _event_sub: None,
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

    pub(crate) fn render_tab_bar(
        &self,
        active_tab: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let can_go_back = self.can_go_back();
        let can_go_forward = self.can_go_forward();

        let tab_data: Vec<(usize, SharedString)> = self
            .open_files
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let name: SharedString = file_name_str(&f.path).into();
                (i, name)
            })
            .collect();

        TabBar::new("editor-tabs")
            .prefix(
                h_flex()
                    .px_1()
                    .gap_0()
                    .child(
                        Button::new("go-back")
                            .ghost()
                            .xsmall()
                            .icon(IconName::ArrowLeft)
                            .disabled(!can_go_back)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.pending_navigation = Some(NavigationAction::GoBack);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("go-forward")
                            .ghost()
                            .xsmall()
                            .icon(IconName::ArrowRight)
                            .disabled(!can_go_forward)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.pending_navigation = Some(NavigationAction::GoForward);
                                cx.notify();
                            })),
                    ),
            )
            .selected_index(active_tab)
            .suffix({
                let handler_entity = self.open_files.get(active_tab).map(|f| f.handler.clone());

                let can_toggle_mode = handler_entity
                    .as_ref()
                    .map(|e| e.read(cx).can_toggle_mode())
                    .unwrap_or(false);

                let is_editing = handler_entity
                    .as_ref()
                    .map(|e| e.read(cx).is_editing())
                    .unwrap_or(false);

                let mut suffix = h_flex().gap_0().px_1();

                if let Some(entity) = handler_entity {
                    if can_toggle_mode {
                        let icon: Icon = if is_editing {
                            Icon::new(IconName::Eye)
                        } else {
                            Icon::default().path(SharedString::from(PEN_ICON))
                        };
                        suffix = suffix.child(
                            Button::new("toggle-mode")
                                .ghost()
                                .xsmall()
                                .icon(icon)
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    entity.update(cx, |handler, cx| handler.toggle_editing(cx));
                                })),
                        );
                    }
                }

                suffix.child(
                    Button::new("new-tab")
                        .ghost()
                        .xsmall()
                        .icon(IconName::Plus)
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.new_empty_tab(cx);
                        })),
                )
            })
            .on_click({
                let tree_state = self.tree_state.clone();
                cx.listener(move |view, index, _, cx| {
                    tree_state.update(cx, |state, cx| {
                        state.set_selected_index(None, cx);
                    });
                    view.last_sidebar_selection = None;
                    view.active_tab = *index;
                    cx.notify();
                })
            })
            .children(tab_data.into_iter().map(|(i, name)| {
                Tab::new().label(name).suffix(
                    Button::new(format!("close-tab-{}", i))
                        .icon(IconName::Close)
                        .ghost()
                        .xsmall()
                        .mx_1()
                        .on_click(cx.listener(move |view, _, _, cx| {
                            cx.stop_propagation();
                            view.close_tab(i, cx);
                        })),
                )
            }))
    }
}
