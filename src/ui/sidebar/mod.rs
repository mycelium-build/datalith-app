pub mod file_tree;
mod navigation;
mod tree_actions;

use std::path::{Path, PathBuf};

use gpui_kit::component::{
    ActiveTheme, Sizable as _,
    button::{Button, ButtonVariants as _},
    input::{InputEvent, InputState},
    menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem},
};
use gpui_kit::{
    AppContext, Context, Focusable, InteractiveElement, IntoElement, KeyDownEvent, MouseDownEvent,
    ParentElement, Pixels, Render, SharedString, Styled, Window, div, prelude::FluentBuilder, px,
};

use crate::app::actions::{
    CopyPath, Delete, Duplicate, NewFile, NewFolder, OpenInExplorer, Rename,
};
use crate::app::settings as app_settings;
use crate::ui::icons::DatalithIcon;
use crate::ui::notifications;
use crate::vault::file_ops;
use crate::vault::path::display_name;

use super::DatalithView;

const BORDER_WIDTH: f32 = 2.0;
const VAULT_MENU_WIDTH_REMS: f32 = 24.0;
const VAULT_MENU_ITEM_CHROME_REMS: f32 = 2.0;
pub(super) const TREE_PADDING_PX: f32 = 12.0;

fn vault_menu_width(window: &Window) -> Pixels {
    px(window.rem_size().as_f32() * VAULT_MENU_WIDTH_REMS)
}

fn vault_menu_row_width(window: &Window) -> Pixels {
    px(window.rem_size().as_f32() * (VAULT_MENU_WIDTH_REMS - VAULT_MENU_ITEM_CHROME_REMS))
}

fn populate_vault_menu(
    mut menu: PopupMenu,
    view: &gpui_kit::Entity<DatalithView>,
    window: &Window,
    cx: &Context<PopupMenu>,
) -> PopupMenu {
    menu = menu.max_w(vault_menu_width(window));
    let docs_path = crate::app::docs::docs_vault_path();
    let docs_view = view.clone();
    menu = menu.item(
        PopupMenuItem::new(crate::app::docs::DOCS_VAULT_NAME)
            .icon(gpui_kit::component::Icon::new(DatalithIcon::Book).size_4())
            .on_click(move |_, window, cx| {
                docs_view.update(cx, |view, cx| {
                    view.set_root_path(docs_path.clone(), None, window, cx);
                });
            }),
    );
    menu = menu.separator();

    let recent_vaults = app_settings::snapshot()
        .recent_vaults
        .into_iter()
        .filter(|path| {
            crate::vault::source::is_dir(path) && !crate::vault::source::is_read_only(path)
        })
        .collect::<Vec<_>>();
    if recent_vaults.is_empty() {
        menu = menu.label("No vault opened");
    } else {
        for path in recent_vaults {
            menu = menu.item(personal_vault_menu_item(path, view, cx));
        }
    }

    menu.item(
        PopupMenuItem::new("Open a new vault").on_click(move |_, window, cx| {
            window.dispatch_action(Box::new(crate::app::actions::OpenVault), cx);
        }),
    )
}

fn personal_vault_menu_item(
    path: PathBuf,
    view: &gpui_kit::Entity<DatalithView>,
    cx: &Context<PopupMenu>,
) -> PopupMenuItem {
    let name: SharedString = display_name(&path).into();
    let close_label = if view.read(cx).root_path.as_ref() == Some(&path) {
        format!("Close and remove {name}")
    } else {
        format!("Remove {name} from recent vaults")
    };
    let button_id = format!("remove-vault-{}", path.display());
    let close_view = view.clone();
    let close_path = path.clone();
    let menu = cx.entity().downgrade();
    let close = std::rc::Rc::new(move |window: &mut Window, cx: &mut gpui_kit::App| {
        window.prevent_default();
        cx.stop_propagation();
        if close_view.update(cx, |view, cx| view.close_personal_vault(&close_path, cx)) {
            // A nested control dismisses its own menu, independent of keyboard focus.
            let _ = menu.update(cx, |_, cx| cx.emit(gpui_kit::DismissEvent));
            close_view
                .read(cx)
                .sidebar_focus_handle
                .clone()
                .focus(window, cx);
        }
    });
    let item_view = view.clone();
    PopupMenuItem::element(move |window, _| {
        let click_close = close.clone();
        let keyboard_close = close.clone();
        let button = Button::new(button_id.clone())
            .icon(gpui_kit::component::IconName::Close)
            .ghost()
            .xsmall()
            .accessibility_label(close_label.clone())
            .on_click(move |_, window, cx| click_close(window, cx))
            // Enter/Space bind to Confirm in the surrounding menu/popover contexts.
            .on_action(move |_: &gpui_kit::base::actions::Confirm, window, cx| {
                keyboard_close(window, cx);
            });
        gpui_kit::component::h_flex()
            .w_full()
            .max_w(vault_menu_row_width(window))
            .min_w_0()
            .items_center()
            .justify_between()
            .child(div().flex_1().min_w_0().truncate().child(name.clone()))
            .child(button.flex_shrink_0())
    })
    .on_click(move |_, window, cx| {
        item_view.update(cx, |view, cx| {
            view.set_root_path(path.clone(), None, window, cx);
        });
    })
}

#[derive(Clone)]
pub struct DragFile {
    pub path: PathBuf,
}

impl Render for DragFile {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let name = display_name(&self.path).to_string();
        div()
            .px_2()
            .py_1()
            .bg(cx.theme().accent)
            .text_color(cx.theme().accent_foreground)
            .rounded_sm()
            .text_sm()
            .border_1()
            .border_color(cx.theme().border)
            .child(name)
    }
}

impl DatalithView {
    pub fn rename_file(
        &mut self,
        old_path: &std::path::Path,
        new_path: &std::path::Path,
        cx: &mut Context<Self>,
    ) {
        if old_path == new_path
            || crate::vault::source::is_read_only(old_path)
            || crate::vault::source::is_read_only(new_path)
        {
            return;
        }
        let Some(catalog) = self.vault_catalog.clone() else {
            return;
        };
        if let Ok(result) = file_ops::rename(&catalog, old_path, new_path) {
            if result.updated_sources == result.total_sources {
                self.pending_notifications
                    .push(notifications::rename_completed(result.updated_sources));
            } else {
                self.pending_notifications
                    .push(notifications::rename_completed_partial(
                        result.updated_sources,
                        result.total_sources,
                    ));
            }
        } else {
            self.pending_notifications
                .push(notifications::rename_failed());
            return;
        }
        self.tabs.rename_path(old_path, new_path);
        if let Some(super::PendingOpen::Open(path) | super::PendingOpen::Created(path)) =
            self.pending_open.as_mut()
            && let Ok(suffix) = path.strip_prefix(old_path)
        {
            *path = new_path.join(suffix);
        }
        self.last_sidebar_selection = self.last_sidebar_selection.as_ref().map(|path| {
            path.strip_prefix(old_path)
                .map_or_else(|_| path.clone(), |suffix| new_path.join(suffix))
        });
        self.refresh_tree_with_rename(old_path, new_path, cx);
    }

    fn handle_file_move(&mut self, old_path: &Path, new_path: &Path, cx: &mut Context<Self>) {
        self.rename_file(old_path, new_path, cx);
    }

    pub(crate) fn commit_rename(&mut self, cx: &mut Context<Self>) {
        if let (Some(ref rename_state), Some(ref target)) =
            (self.rename_state.clone(), self.rename_target.clone())
        {
            let new_name = rename_state.read(cx).value().to_string();
            if !new_name.is_empty() {
                let old_ext = if crate::vault::source::is_dir(target) {
                    None
                } else {
                    target
                        .file_name()
                        .and_then(|n| n.to_str())
                        .and_then(|name| {
                            name.rfind('.')
                                .filter(|&dot| dot > 0)
                                .and_then(|dot| name.get(dot..))
                                .map(ToString::to_string)
                        })
                };
                let mut name = new_name;
                if let Some(ref ext) = old_ext
                    && !name.contains('.')
                {
                    name.push_str(ext);
                }
                if let Some(parent) = target.parent() {
                    let candidate = parent.join(&name);
                    if candidate != *target {
                        let final_path = file_ops::unique_name(parent, &name);
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
        self.rename_sub = None;
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

        let current = display_name(target).to_string();
        let state = cx.new(|cx| InputState::new(window, cx).default_value(current.as_str()));

        self.rename_sub = Some(cx.subscribe_in(
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
        window.dispatch_action(Box::new(gpui_kit::component::input::SelectAll), cx);
    }

    fn path_from_id(id: &SharedString) -> PathBuf {
        PathBuf::from(id.to_string())
    }

    pub fn render_sidebar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let view = cx.entity();

        self.ensure_rename_state(window, cx);

        let selector_view = cx.entity();
        let active_read_only = self
            .root_path
            .as_deref()
            .is_some_and(crate::vault::source::is_read_only);
        let selector_label = if self.root_path.is_some() {
            self.root_name.clone()
        } else {
            SharedString::from("No vault opened")
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .h_full()
            .bg(cx.theme().tab_bar)
            .border_r(px(BORDER_WIDTH))
            .border_color(cx.theme().border)
            .track_focus(&self.sidebar_focus_handle)
            .on_mouse_down(
                gpui_kit::MouseButton::Left,
                cx.listener(|this, _event: &MouseDownEvent, window, cx| {
                    this.sidebar_focus_handle.focus(window, cx);
                    cx.stop_propagation();
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.on_sidebar_key_down(event, window, cx);
            }))
            .when(!active_read_only, |sidebar| {
                sidebar.on_drop(cx.listener(move |this, drag: &DragFile, _window, cx| {
                    if let (Some(root), Some(name)) = (&this.root_path, drag.path.file_name()) {
                        let new_path = root.join(name);
                        this.handle_file_move(&drag.path, &new_path, cx);
                    }
                }))
            })
            .child(
                div()
                    .flex_1()
                    .child(Self::render_file_tree(cx, &self.tree_state)),
            )
            .child(
                div()
                    .border_t(px(BORDER_WIDTH))
                    .border_color(cx.theme().border)
                    .p_2()
                    .child(
                        Button::new("vault-selector")
                            .ghost()
                            .small()
                            .w_full()
                            .label(selector_label)
                            .dropdown_caret(true)
                            .accessibility_label("Select vault")
                            .dropdown_menu_with_anchor(
                                gpui_kit::Anchor::BottomLeft,
                                move |menu, window, cx| {
                                    populate_vault_menu(menu, &selector_view, window, cx)
                                },
                            ),
                    ),
            )
            .context_menu(move |menu, _window, cx| {
                let from_row = view.update(cx, |v, _| {
                    let from_row = v.context_menu_from_row;
                    v.context_menu_from_row = false;
                    from_row
                });

                if !from_row {
                    view.update(cx, |v, _| v.context_menu_target = v.root_path.clone());
                }

                menu.menu_with_disabled("New File", Box::new(NewFile), active_read_only)
                    .menu_with_disabled("New Folder", Box::new(NewFolder), active_read_only)
                    .when(from_row, |menu| {
                        menu.separator()
                            .menu_with_disabled("Rename", Box::new(Rename), active_read_only)
                            .menu_with_disabled("Delete", Box::new(Delete), active_read_only)
                            .menu_with_disabled("Duplicate", Box::new(Duplicate), active_read_only)
                    })
                    .separator()
                    .menu_with_disabled(
                        "Open in Explorer",
                        Box::new(OpenInExplorer),
                        active_read_only,
                    )
                    .menu("Copy Path", Box::new(CopyPath))
            })
    }
}
