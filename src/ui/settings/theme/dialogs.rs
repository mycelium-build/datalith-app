use std::{cell::RefCell, path::Path, rc::Rc};

use gpui_kit::component::{
    ActiveTheme as _, Sizable as _, WindowExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputState},
    v_flex,
};
use gpui_kit::{App, AppContext as _, Focusable as _, ParentElement, Styled as _, Window, div};

use crate::app::themes::{PreparedThemeImport, ThemeSource};

use super::{ImportPolicy, ThemeLibrary, notifications, open_editor, themes};

/// Focused form whose retained input and inline validation outlive dialog renders.
fn name_form(
    title: String,
    description: String,
    default: String,
    action: &'static str,
    window: &mut Window,
    cx: &mut App,
    commit: impl Fn(&str, &mut Window, &mut App) -> anyhow::Result<()> + 'static,
) {
    let name = cx.new(|cx| InputState::new(window, cx).default_value(default));
    let error = Rc::new(RefCell::<Option<String>>::new(None));
    let commit = Rc::new(commit);
    let focus = name.focus_handle(cx);
    window.open_dialog(cx, move |dialog, _, _| {
        let description = description.clone();
        let commit = commit.clone();
        let name_content = name.clone();
        let error_content = error.clone();
        let name_commit = name.clone();
        let error_commit = error.clone();
        dialog
            .title(title.clone())
            .footer(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| {
                                window
                                    .dispatch_action(Box::new(gpui_kit::base::actions::Cancel), cx);
                            }),
                    )
                    .child(
                        Button::new("ok")
                            .primary()
                            .label(action)
                            .on_click(|_, window, cx| {
                                window.dispatch_action(
                                    Box::new(gpui_kit::base::actions::Confirm { secondary: false }),
                                    cx,
                                );
                            }),
                    ),
            )
            .content(move |content, _, cx| {
                content.child(
                    v_flex()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(description.clone()),
                        )
                        .child(div().text_sm().child("Name"))
                        .child(
                            Input::new(&name_content)
                                .id("theme-dialog-name")
                                .small()
                                .aria_label("Name"),
                        )
                        .children(error_content.borrow().as_ref().map(|error| {
                            div()
                                .text_sm()
                                .text_color(cx.theme().danger)
                                .child(error.clone())
                        })),
                )
            })
            .on_ok(move |_, window, cx| {
                let value = name_commit.read(cx).value();
                match commit(value.as_str(), window, cx) {
                    Ok(()) => true,
                    Err(failure) => {
                        *error_commit.borrow_mut() = Some(failure.to_string());
                        cx.refresh_windows();
                        false
                    }
                }
            })
    });
    window.defer(cx, move |window, cx| focus.focus(window, cx));
}

pub(super) fn copy_family(id: u64, window: &mut Window, cx: &mut App) {
    let Some(family) = cx.global::<ThemeLibrary>().family(id) else {
        return;
    };
    let title = format!("Copy “{}”", family.name());
    let description = format!(
        "Creates a custom theme with its {} variants.",
        family.variants().len()
    );
    let default = cx
        .global::<ThemeLibrary>()
        .suggested_copy_name(family.name());
    name_form(
        title,
        description,
        default,
        "Copy & edit",
        window,
        cx,
        move |name, window, cx| {
            let id = cx.global_mut::<ThemeLibrary>().copy_family(id, name)?;
            open_editor(id, None, window, cx);
            Ok(())
        },
    );
}

pub fn rename_family(id: u64, window: &mut Window, cx: &mut App) {
    let Some(family) = cx.global::<ThemeLibrary>().family(id) else {
        return;
    };
    let title = format!("Rename “{}”", family.name());
    name_form(
        title,
        "The variants and their colors stay together under the new name.".into(),
        family.name().into(),
        "Rename",
        window,
        cx,
        move |name, window, cx| {
            cx.global_mut::<ThemeLibrary>().rename_family(id, name)?;
            themes::refresh_current(cx);
            if let Some(view) = cx
                .try_global::<crate::app::AppState>()
                .and_then(|state| state.view.clone())
                && let Some(editor) = view.read(cx).tabs.theme_editor_for(id, cx).cloned()
            {
                editor.update(cx, |editor, cx| editor.refresh_variants(window, cx));
            }
            cx.refresh_windows();
            Ok(())
        },
    );
}

pub fn import_family(path: &Path, cx: &mut App) {
    let prepared = match cx.global::<ThemeLibrary>().prepare_import(path) {
        Ok(prepared) => Rc::new(prepared),
        Err(error) => {
            notifications::push_window_notification(
                cx,
                notifications::settings_save_failed("theme import", &error),
            );
            return;
        }
    };
    let Some(existing) = prepared
        .conflict()
        .and_then(|id| cx.global::<ThemeLibrary>().family(id))
    else {
        apply_import(&prepared, ImportPolicy::Copy, None, cx);
        return;
    };
    let replace = matches!(existing.source(), ThemeSource::Custom(_));
    let title = format!("Import “{}”", existing.name());
    if let Some(window) = cx.active_window() {
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                let copy = prepared.clone();
                let replacement = prepared.clone();
                window.open_dialog(cx, move |dialog, _, _| {
                    let copy = copy.clone();
                    let replacement = replacement.clone();
                    dialog
                        .title(title.clone())
                        .content(move |content, _, cx| {
                            content.child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("A theme with this name already exists"),
                            )
                        })
                        .on_ok(move |_, window, cx| {
                            apply_import(&copy, ImportPolicy::Copy, Some(window), cx);
                            true
                        })
                        .footer(
                            h_flex()
                                .gap_2()
                                .child(
                                    Button::new("cancel-theme-import")
                                        .label("Cancel")
                                        .on_click(|_, window, cx| window.close_dialog(cx)),
                                )
                                .children(replace.then(|| {
                                    Button::new("replace-theme-import")
                                        .label("Replace")
                                        .on_click(move |_, window, cx| {
                                            apply_import(
                                                &replacement,
                                                ImportPolicy::Replace,
                                                Some(window),
                                                cx,
                                            );
                                            window.close_dialog(cx);
                                        })
                                }))
                                .child(
                                    Button::new("copy-theme-import")
                                        .primary()
                                        .label("Import as copy")
                                        .on_click(|_, window, cx| {
                                            window.dispatch_action(
                                                Box::new(gpui_kit::base::actions::Confirm {
                                                    secondary: false,
                                                }),
                                                cx,
                                            );
                                        }),
                                ),
                        )
                });
            });
        });
    }
}

fn apply_import(
    prepared: &PreparedThemeImport,
    policy: ImportPolicy,
    window: Option<&mut Window>,
    cx: &mut App,
) {
    match cx
        .global_mut::<ThemeLibrary>()
        .import_prepared(prepared, policy)
    {
        Ok(id) => {
            themes::refresh_current(cx);
            if matches!(policy, ImportPolicy::Replace)
                && let Some(window) = window
                && let Some(view) = cx
                    .try_global::<crate::app::AppState>()
                    .and_then(|state| state.view.clone())
                && let Some(editor) = view.read(cx).tabs.theme_editor_for(id, cx).cloned()
            {
                editor.update(cx, |editor, cx| editor.reload_variants(window, cx));
            }
            cx.refresh_windows();
        }
        Err(error) => {
            let notification = notifications::settings_save_failed("theme import", &error);
            if let Some(window) = window {
                window.push_notification(notification, cx);
            } else {
                notifications::push_window_notification(cx, notification);
            }
        }
    }
}
