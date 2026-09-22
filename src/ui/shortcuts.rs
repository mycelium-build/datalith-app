//! Keyboard shortcut reference in a dedicated workspace tab.

use gpui_kit::base::TestSupportExt as _;
use gpui_kit::component::{
    ActiveTheme, Sizable as _, Size, h_flex,
    setting::{SettingGroup, SettingItem, SettingPage, Settings},
    v_flex,
};
use gpui_kit::{
    App, Context, FocusHandle, Focusable, InteractiveElement as _, IntoElement, ParentElement,
    Render, SharedString, Styled, Window, div,
};

pub struct ShortcutsView {
    focus: FocusHandle,
}

impl ShortcutsView {
    pub(crate) fn new(cx: &Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
        }
    }
}

impl Focusable for ShortcutsView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for ShortcutsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("shortcuts-editor")
            .size_full()
            .min_h_0()
            .test_support()
            .track_focus(&self.focus)
            .bg(cx.theme().background)
            .child(
                div()
                    .p_4()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .text_lg()
                    .child("Keyboard shortcuts"),
            )
            .child(
                Settings::new("app-shortcuts")
                    .with_size(Size::Small)
                    .pages(vec![
                        SettingPage::new("Shortcuts").groups(Self::shortcuts_groups()),
                    ]),
            )
    }
}

fn merged_shortcut_rows() -> Vec<(SharedString, SharedString, SharedString)> {
    struct Run {
        category: SharedString,
        description: SharedString,
        first_keys: SharedString,
        last_keys: SharedString,
    }

    let descriptions = crate::app::keymap::shortcut_descriptions();

    // Merge consecutive shortcuts that share a category and description into a range
    let mut runs: Vec<Run> = Vec::new();
    for (category, keys, description) in descriptions {
        let keys = crate::app::keymap::display_binding(keys);
        let extends = runs.last().is_some_and(|run| {
            run.category.as_str() == category && run.description.as_str() == description
        });
        if extends && let Some(run) = runs.last_mut() {
            run.last_keys = keys.into();
        } else {
            runs.push(Run {
                category: category.into(),
                description: description.into(),
                first_keys: keys.clone().into(),
                last_keys: keys.into(),
            });
        }
    }

    runs.into_iter()
        .map(|run| {
            let keys = if run.first_keys == run.last_keys {
                run.first_keys
            } else {
                format!("{} … {}", run.first_keys, run.last_keys).into()
            };
            (run.category, keys, run.description)
        })
        .collect()
}

impl ShortcutsView {
    #[allow(clippy::too_many_lines)]
    fn shortcuts_groups() -> Vec<SettingGroup> {
        let merged = merged_shortcut_rows();

        let mut groups: Vec<(SharedString, Vec<(SharedString, SharedString)>)> = Vec::new();
        for (category, keys, description) in merged {
            match groups.last_mut() {
                Some((cat, rows)) if cat.as_str() == category.as_str() => {
                    rows.push((keys, description));
                }
                _ => groups.push((category, vec![(keys, description)])),
            }
        }

        groups
            .into_iter()
            .map(|(category, rows)| {
                SettingGroup::new()
                    .title(category)
                    .items(vec![SettingItem::render(move |_options, _window, cx| {
                        v_flex()
                            .w_full()
                            .gap_1()
                            .children(rows.iter().map(|(keys, description)| {
                                h_flex()
                                    .w_full()
                                    .justify_between()
                                    .gap_4()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(description.clone()),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(cx.theme().muted)
                                            .text_sm()
                                            .child(keys.clone()),
                                    )
                                    .into_any_element()
                            }))
                            .into_any_element()
                    })])
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::merged_shortcut_rows;

    #[test]
    fn shortcut_rows_merge_select_tab_range_with_displayed_bindings() {
        let secondary = if cfg!(target_os = "macos") {
            "⌘"
        } else {
            "ctrl-"
        };
        let expected_keys = format!("{secondary}1 … {secondary}8");

        assert!(
            merged_shortcut_rows()
                .iter()
                .any(|(category, keys, description)| {
                    category.as_str() == "Tabs"
                        && keys.as_str() == expected_keys
                        && description.as_str() == "Select tab"
                })
        );
    }
}
