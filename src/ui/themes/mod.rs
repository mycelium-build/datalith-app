mod editor;
pub use editor::ThemeEditor;

use crate::ui::notifications;

/// Only the About mark identifies the release channel by color.
pub fn about_logo_color(theme: &gpui_kit::component::Theme) -> gpui_kit::Hsla {
    match crate::channel::Channel::current() {
        crate::channel::Channel::Stable => theme.primary,
        crate::channel::Channel::Preview => gpui_kit::rgb(0xe8_b9_20).into(),
        crate::channel::Channel::Dev => gpui_kit::rgb(0x30_ba_78).into(),
    }
}

#[derive(Clone)]
pub enum ThemeChange {
    Select { name: String, activate: bool },
    Mode(crate::app::settings::ThemePreference),
}

pub fn select_theme(name: &str, activate: bool, cx: &mut gpui_kit::App) {
    request_change(
        ThemeChange::Select {
            name: name.to_owned(),
            activate,
        },
        cx,
    );
}

pub fn change_mode(preference: crate::app::settings::ThemePreference, cx: &mut gpui_kit::App) {
    request_change(ThemeChange::Mode(preference), cx);
}

fn request_change(change: ThemeChange, cx: &mut gpui_kit::App) {
    let editor = cx
        .try_global::<crate::app::AppState>()
        .and_then(|state| state.view.as_ref())
        .and_then(|view| view.read(cx).tabs.theme_editor().cloned());
    if let Some(editor) = editor
        && let Some(window) = cx.active_window()
    {
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                if editor.read(cx).has_unsaved_changes()
                    && let Some(view) = cx.global::<crate::app::AppState>().view.clone()
                {
                    view.update(cx, |view, cx| view.open_theme_editor(window, cx));
                }
                editor.update(cx, |editor, cx| editor.request_change(change, window, cx));
            });
        });
    } else {
        apply_change(change, cx);
    }
}

fn apply_change(change: ThemeChange, cx: &mut gpui_kit::App) {
    match change {
        ThemeChange::Select { name, activate } => {
            apply_selection(&name, activate, cx);
        }
        ThemeChange::Mode(preference) => {
            if let Err(error) = crate::app::settings::set_theme_preference(preference) {
                notifications::push_window_notification(
                    cx,
                    notifications::settings_save_failed("theme mode", &error),
                );
                return;
            }
            crate::app::preferences::apply_theme_preference(preference, cx);
        }
    }
}

/// Apply the selection after resolving any open draft, then offer theme fonts.
fn apply_selection(name: &str, activate: bool, cx: &mut gpui_kit::App) -> bool {
    if let Err(error) = crate::app::themes::select(name, activate, cx) {
        notifications::push_window_notification(
            cx,
            notifications::settings_save_failed("theme", &error),
        );
        return false;
    }
    let has_fonts = crate::app::themes::document(name, cx).is_some_and(|theme| theme.has_fonts());
    if has_fonts
        && crate::app::fonts::has_personal_fonts(cx)
        && let Some(window) = cx.active_window()
    {
        use gpui_kit::component::{WindowExt as _, dialog::DialogButtonProps};
        use std::ops::Mul as _;
        let name = name.to_owned();
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                window.open_alert_dialog(cx, move |dialog, window, _| {
                    dialog
                        .title("Use theme fonts?")
                        .description(format!(
                            "{name} defines its own fonts. Keep your personal fonts or use the theme fonts for all four roles."
                        ))
                        .width((window.rem_size().mul(30.)).min(window.viewport_size().width))
                        .button_props(DialogButtonProps::default()
                            .ok_text("Use theme fonts")
                            .cancel_text("Keep personal fonts")
                            .show_cancel(true))
                        .on_ok(|_, window, cx| {
                            if let Err(error) = crate::app::fonts::use_theme_fonts(cx) {
                                window.push_notification(notifications::settings_save_failed("fonts", &error), cx);
                                return false;
                            }
                            true
                        })
                });
            });
        });
    }
    true
}
