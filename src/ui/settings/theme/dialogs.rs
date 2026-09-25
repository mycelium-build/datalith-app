use std::{cell::RefCell, path::PathBuf, rc::Rc};

use gpui_kit::component::{
    ActiveTheme as _, Sizable as _, WindowExt as _,
    button::{Button, ButtonVariants as _},
    dialog::DialogButtonProps,
    h_flex,
    input::Input,
    v_flex,
};
use gpui_kit::{App, AppContext as _, Focusable as _, ParentElement, Styled as _, Window, div};

use super::{ImportPolicy, SettingsView, ThemeLibrary, notifications, open_editor, themes};

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
    let name =
        cx.new(|cx| gpui_kit::component::input::InputState::new(window, cx).default_value(default));
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
            .button_props(
                DialogButtonProps::default()
                    .ok_text(action)
                    .cancel_text("Cancel")
                    .show_cancel(true),
            )
            .footer(form_actions(action))
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

pub(super) fn rename_family(id: u64, window: &mut Window, cx: &mut App) {
    let Some(family) = cx.global::<ThemeLibrary>().family(id) else {
        return;
    };
    let title = format!("Rename “{}”", family.name());
    name_form(
        title,
        "Updates the prefix of every variant".into(),
        family.name().into(),
        "Rename",
        window,
        cx,
        move |name, window, cx| {
            cx.global_mut::<ThemeLibrary>().rename_family(id, name)?;
            themes::refresh_current(cx);
            SettingsView::init_theme_options(cx);
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

#[allow(
    clippy::too_many_lines,
    reason = "The two retained form fields and validation share a single dialog lifecycle"
)]
pub fn name_variants(family_id: u64, from: u64, window: &mut Window, cx: &mut App) {
    let Some(family) = cx.global::<ThemeLibrary>().family(family_id) else {
        return;
    };
    let first = family
        .suffix(from)
        .filter(|s| !s.is_empty())
        .unwrap_or("Variant 1")
        .to_owned();
    let second = if first.eq_ignore_ascii_case("Variant 1") {
        "Variant 2"
    } else {
        "Variant 1"
    };
    let first_input =
        cx.new(|cx| gpui_kit::component::input::InputState::new(window, cx).default_value(first));
    let second_input =
        cx.new(|cx| gpui_kit::component::input::InputState::new(window, cx).default_value(second));
    let error = Rc::new(RefCell::<Option<String>>::new(None));
    let focus = first_input.focus_handle(cx);
    window.open_dialog(cx, move |dialog, _, _| {
        let first_content = first_input.clone();
        let second_content = second_input.clone();
        let first_commit = first_input.clone();
        let second_commit = second_input.clone();
        let error_content = error.clone();
        let error_commit = error.clone();
        dialog
            .title("Name the variants")
            .button_props(
                DialogButtonProps::default()
                    .ok_text("Add variant")
                    .cancel_text("Cancel")
                    .show_cancel(true),
            )
            .footer(form_actions("Add variant"))
            .content(move |content, _, cx| {
                content.child(
                    v_flex()
                        .gap_3()
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("Give each variant a unique suffix"),
                        )
                        .child(div().child("Existing variant"))
                        .child(
                            Input::new(&first_content)
                                .id("existing-variant-suffix")
                                .small()
                                .aria_label("Existing variant suffix"),
                        )
                        .child(div().child("New variant"))
                        .child(
                            Input::new(&second_content)
                                .id("new-variant-suffix")
                                .small()
                                .aria_label("New variant suffix"),
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
                let first = first_commit.read(cx).value().to_string();
                let second = second_commit.read(cx).value().to_string();
                match cx.global_mut::<ThemeLibrary>().add_variant(
                    family_id,
                    from,
                    &second,
                    Some(&first),
                ) {
                    Ok(_) => {
                        ThemeLibrary::schedule_save(family_id, cx);
                        themes::refresh_current(cx);
                        SettingsView::init_theme_options(cx);
                        if let Some(view) = cx
                            .try_global::<crate::app::AppState>()
                            .and_then(|s| s.view.clone())
                        {
                            let editor =
                                view.read(cx).tabs.theme_editor_for(family_id, cx).cloned();
                            if let Some(editor) = editor {
                                editor.update(cx, |editor, cx| editor.refresh_variants(window, cx));
                            }
                        }
                        true
                    }
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

fn form_actions(label: &'static str) -> impl gpui_kit::IntoElement {
    h_flex()
        .gap_2()
        .child(
            Button::new("cancel")
                .label("Cancel")
                .on_click(|_, window, cx| {
                    window.dispatch_action(Box::new(gpui_kit::base::actions::Cancel), cx);
                }),
        )
        .child(
            Button::new("ok")
                .primary()
                .label(label)
                .on_click(|_, window, cx| {
                    window.dispatch_action(
                        Box::new(gpui_kit::base::actions::Confirm { secondary: false }),
                        cx,
                    );
                }),
        )
}

pub fn import_family(path: PathBuf, cx: &mut App) {
    let name = std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|value| value.get("name")?.as_str().map(str::to_owned));
    let Some(existing) = name.as_deref().and_then(|name| {
        cx.global::<ThemeLibrary>()
            .families()
            .find(|family| family.name().eq_ignore_ascii_case(name))
    }) else {
        apply_import(&path, ImportPolicy::Copy, None, cx);
        return;
    };
    let replace = matches!(
        existing.source(),
        crate::app::themes::ThemeSource::Custom(_)
    );
    let title = format!("Import “{}”", existing.name());
    if let Some(window) = cx.active_window() {
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                let copy_path = path.clone();
                let replacement = path.clone();
                window.open_dialog(cx, move |dialog, _, _| {
                    let copy_path = copy_path.clone();
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
                            apply_import(&copy_path, ImportPolicy::Copy, Some(window), cx);
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
    path: &std::path::Path,
    policy: ImportPolicy,
    window: Option<&mut Window>,
    cx: &mut App,
) {
    match cx.global_mut::<ThemeLibrary>().import(path, policy) {
        Ok(id) => {
            themes::refresh_current(cx);
            SettingsView::init_theme_options(cx);
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
