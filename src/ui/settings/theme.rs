//! Theme management in its own modal. Family and variant identity comes from the library.

use std::collections::HashMap;

use gpui_kit::base::TestSupportExt as _;
use gpui_kit::component::{
    ActiveTheme as _, IconName, Selectable as _, Sizable as _, WindowExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu as _, PopupMenuItem},
    notification::Notification,
    select::{Select, SelectEvent, SelectState},
    v_flex,
};
use gpui_kit::{
    App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ListAlignment,
    ListState, ParentElement, PathPromptOptions, Pixels, Render, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window, div, list, px, rems,
};

use super::SettingsView;
use crate::app::{
    fonts::FontCatalog,
    settings::{self, FontRole, ThemeKind, ThemePreference},
    themes::{self, DeletedTheme, ImportPolicy, ThemeFamily, ThemeLibrary, ThemeSource},
};
use crate::ui::{notifications, themes as ui_themes};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Light,
    Dark,
    Custom,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FamilyRow {
    Heading(&'static str),
    Family(u64),
}

struct VariantSummary {
    colors: Vec<gpui_kit::Hsla>,
    fonts: [gpui_kit::SharedString; 3],
}

pub(super) struct ThemePage {
    query: Entity<InputState>,
    mode: Entity<SelectState<Vec<String>>>,
    filter: Filter,
    summaries: HashMap<u64, VariantSummary>,
    family_revisions: HashMap<u64, u64>,
    _library_subscription: Subscription,
    rows: Vec<FamilyRow>,
    list: ListState,
    rem_size: Pixels,
    _query_subscription: Subscription,
    _mode_subscription: Subscription,
}

impl ThemePage {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("Search themes"));
        let subscription = cx.subscribe_in(&query, window, |_, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        let preferences = [
            ThemePreference::System,
            ThemePreference::Light,
            ThemePreference::Dark,
        ];
        let selected = preferences
            .iter()
            .position(|preference| *preference == settings::snapshot().theme_preference)
            .unwrap_or(0);
        let mode = cx.new(|cx| {
            SelectState::new(
                vec!["System".into(), "Light".into(), "Dark".into()],
                Some(gpui_kit::component::IndexPath::default().row(selected)),
                window,
                cx,
            )
        });
        let mode_subscription = cx.subscribe_in(
            &mode,
            window,
            |_, _, event: &SelectEvent<Vec<String>>, _, cx| {
                if let SelectEvent::Confirm(Some(value)) = event
                    && let Some(preference) = (match value.as_str() {
                        "System" => Some(ThemePreference::System),
                        "Light" => Some(ThemePreference::Light),
                        "Dark" => Some(ThemePreference::Dark),
                        _ => None,
                    })
                {
                    ui_themes::change_mode(preference, cx);
                }
            },
        );
        let library_subscription = cx.observe_global::<ThemeLibrary>(|this, cx| {
            this.refresh_summaries(cx);
            this.list.remeasure();
            cx.notify();
        });
        let mut page = Self {
            query,
            mode,
            filter: Filter::All,
            summaries: HashMap::new(),
            family_revisions: HashMap::new(),
            _library_subscription: library_subscription,
            rows: Vec::new(),
            list: ListState::new(0, ListAlignment::Top, px(100.)),
            rem_size: window.rem_size(),
            _query_subscription: subscription,
            _mode_subscription: mode_subscription,
        };
        page.refresh_summaries(cx);
        page
    }

    fn matches(&self, family: &ThemeFamily, query: &str) -> bool {
        let source_matches =
            self.filter != Filter::Custom || matches!(family.source(), ThemeSource::Custom(_));
        let mode_matches = match self.filter {
            Filter::Light => family
                .variants()
                .iter()
                .any(|v| v.mode() == gpui_kit::component::ThemeMode::Light),
            Filter::Dark => family
                .variants()
                .iter()
                .any(|v| v.mode() == gpui_kit::component::ThemeMode::Dark),
            Filter::All | Filter::Custom => true,
        };
        source_matches
            && mode_matches
            && (family.name().to_lowercase().contains(query)
                || family
                    .variants()
                    .iter()
                    .any(|v| v.name().to_lowercase().contains(query)))
    }

    fn refresh_summaries(&mut self, cx: &App) {
        let library = cx.global::<ThemeLibrary>();
        self.family_revisions
            .retain(|id, _| library.family(*id).is_some());
        self.summaries
            .retain(|id, _| library.variant_by_id(*id).is_some());
        for family in library.families() {
            if self.family_revisions.get(&family.id()) == Some(&family.revision()) {
                continue;
            }
            for variant in family.variants() {
                if let Ok(appearance) = library.resolved(variant.id(), cx.global::<FontCatalog>()) {
                    let theme = appearance.theme();
                    self.summaries.insert(
                        variant.id(),
                        VariantSummary {
                            colors: vec![
                                theme.background,
                                theme.foreground,
                                theme.primary,
                                theme.accent,
                                theme.border,
                                theme.success,
                            ],
                            fonts: [FontRole::Reading, FontRole::Headings, FontRole::Code]
                                .map(|role| appearance.font(role).clone()),
                        },
                    );
                }
            }
            self.family_revisions.insert(family.id(), family.revision());
        }
    }

    fn render_swatches(&self, id: u64, cx: &Context<Self>) -> impl IntoElement {
        h_flex().gap_1().children(
            self.summaries
                .get(&id)
                .into_iter()
                .flat_map(|summary| summary.colors.iter())
                .map(|color| {
                    div()
                        .size_4()
                        .rounded_sm()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(*color)
                }),
        )
    }

    fn render_current(&self, kind: ThemeKind, cx: &Context<Self>) -> impl IntoElement {
        let library = cx.global::<ThemeLibrary>();
        let name = library.current(kind);
        let id = library
            .variant(name)
            .map(crate::app::themes::ThemeVariant::id);
        v_flex()
            .flex_1()
            .min_w_0()
            .p_3()
            .gap_2()
            .rounded_md()
            .bg(cx.theme().muted)
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(if kind == ThemeKind::Light {
                        "Current light theme"
                    } else {
                        "Current dark theme"
                    }),
            )
            .child(
                h_flex()
                    .gap_3()
                    .child(div().flex_1().min_w_0().truncate().child(name.to_owned()))
                    .children(id.map(|id| self.render_swatches(id, cx))),
            )
    }

    fn render_variant(
        &self,
        variant: &crate::app::themes::ThemeVariant,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let id = variant.id();
        let kind = ThemeKind::from(variant.mode());
        let current = cx.global::<ThemeLibrary>().current(kind) == variant.name();
        let light = kind == ThemeKind::Light;
        let action = if current {
            if light {
                "Current light theme ✓"
            } else {
                "Current dark theme ✓"
            }
        } else if light {
            "Set as light theme"
        } else {
            "Set as dark theme"
        };
        v_flex()
            .w_full()
            .min_w_0()
            .gap_2()
            .px_3()
            .py_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .w_full()
                    .gap_3()
                    .flex_wrap()
                    .child(
                        div()
                            .w(rems(3.5))
                            .flex_none()
                            .text_sm()
                            .text_color(cx.theme().foreground)
                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                            .child(if light { "Light" } else { "Dark" }),
                    )
                    .child(div().flex_1().min_w_0().child(variant.name().to_owned()))
                    .child(self.render_swatches(id, cx))
                    .child(if current {
                        h_flex()
                            .id(("current-theme", id))
                            .test_support()
                            .role(gpui_kit::Role::Status)
                            .aria_label(format!(
                                "{} is set as the {} theme",
                                variant.name(),
                                if light { "light" } else { "dark" }
                            ))
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_sm()
                            .bg(cx.theme().accent)
                            .text_color(cx.theme().accent_foreground)
                            .child(action)
                            .into_any_element()
                    } else {
                        Button::new(("set-current", id))
                            .small()
                            .label(action)
                            .accessibility_label(format!(
                                "Set {} as the {} theme",
                                variant.name(),
                                if light { "light" } else { "dark" }
                            ))
                            .on_click(move |_, _, cx| ui_themes::set_current(id, kind, cx))
                            .into_any_element()
                    }),
            )
            .children(self.summaries.get(&id).map(|summary| {
                h_flex()
                    .gap_4()
                    .flex_wrap()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .children(
                        ["Reading", "Headings", "Code"]
                            .into_iter()
                            .zip(summary.fonts.iter())
                            .map(|(role, font)| {
                                h_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                                            .child(format!("{role}:")),
                                    )
                                    .child(
                                        div()
                                            .text_color(cx.theme().foreground)
                                            .font_family(font.clone())
                                            .child(if font.as_ref() == ".SystemUIFont" {
                                                "System font".into()
                                            } else {
                                                font.clone()
                                            }),
                                    )
                            }),
                    )
            }))
    }

    fn render_family(&self, family: &ThemeFamily, cx: &Context<Self>) -> impl IntoElement {
        let id = family.id();
        let custom = matches!(family.source(), ThemeSource::Custom(_));
        let name = family.name().to_owned();
        v_flex()
            .id(("theme-family", id))
            .w_full()
            .min_w_0()
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .gap_2()
                    .py_2()
                    .child(div().flex_1().min_w_0().truncate().child(name.clone()))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{} variants", family.variants().len())),
                    )
                    .child(if custom {
                        Button::new(("edit-theme", id))
                            .small()
                            .label("Customize…")
                            .on_click(move |_, window, cx| open_editor(id, None, window, cx))
                    } else {
                        Button::new(("copy-theme", id))
                            .small()
                            .label("Copy & customize…")
                            .on_click(move |_, window, cx| {
                                dialogs::copy_family(id, window, cx);
                            })
                    })
                    .children(custom.then(|| {
                        Button::new(("theme-actions", id))
                            .small()
                            .ghost()
                            .icon(IconName::Ellipsis)
                            .tooltip(format!("Actions for {name}"))
                            .accessibility_label(format!("Actions for {name}"))
                            .dropdown_menu(move |menu, _, _| {
                                menu.item(PopupMenuItem::new("Rename").on_click(
                                    move |_, window, cx| dialogs::rename_family(id, window, cx),
                                ))
                                .item(
                                    PopupMenuItem::new("Export")
                                        .on_click(move |_, _, cx| export_family(id, cx)),
                                )
                                .separator()
                                .item(
                                    PopupMenuItem::element(|_, cx| {
                                        div().text_color(cx.theme().danger).child("Delete")
                                    })
                                    .on_click(move |_, window, cx| delete_family(id, window, cx)),
                                )
                            })
                    })),
            )
            .child(
                v_flex().children(
                    family
                        .variants()
                        .iter()
                        .filter(|variant| match self.filter {
                            Filter::Light => !variant.mode().is_dark(),
                            Filter::Dark => variant.mode().is_dark(),
                            Filter::All | Filter::Custom => true,
                        })
                        .map(|variant| self.render_variant(variant, cx)),
                ),
            )
    }
}

impl ThemePage {
    pub(super) fn sync_mode(&self, window: &mut Window, cx: &mut Context<Self>) {
        let selected = match settings::snapshot().theme_preference {
            ThemePreference::System => "System",
            ThemePreference::Light => "Light",
            ThemePreference::Dark => "Dark",
        };
        if self
            .mode
            .read(cx)
            .selected_value()
            .is_none_or(|value| value != selected)
        {
            self.mode.update(cx, |mode, cx| {
                mode.set_selected_value(&selected.to_owned(), window, cx);
            });
        }
    }

    fn refresh_rows(&mut self, window: &Window, cx: &App) {
        let query = self.query.read(cx).value().to_lowercase();
        let (custom, bundled): (Vec<_>, Vec<_>) = cx
            .global::<ThemeLibrary>()
            .families()
            .filter(|family| self.matches(family, &query))
            .partition(|family| matches!(family.source(), ThemeSource::Custom(_)));
        let mut rows = Vec::new();
        if !custom.is_empty() {
            rows.push(FamilyRow::Heading("Custom"));
            rows.extend(custom.iter().map(|family| FamilyRow::Family(family.id())));
        }
        if !bundled.is_empty() {
            rows.push(FamilyRow::Heading("Built-in"));
            rows.extend(bundled.iter().map(|family| FamilyRow::Family(family.id())));
        }
        if self.rows != rows {
            self.list.reset(rows.len());
            self.rows = rows;
        } else if self.rem_size != window.rem_size() {
            self.list.remeasure();
            self.rem_size = window.rem_size();
        }
    }
}

impl Render for ThemePage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh_rows(window, cx);
        let rows = self.rows.clone();
        let empty = rows.is_empty();
        let page = cx.entity();
        v_flex()
            .id("theme-settings-page")
            .min_w_0()
            .size_full()
            .min_h_0()
            .gap_4()
            .child(
                h_flex()
                    .justify_between()
                    .gap_3()
                    .child("Appearance")
                    .child(
                        Select::new(&self.mode)
                            .id("theme-appearance")
                            .small()
                            .w(rems(14.))
                            .accessibility_label("Appearance mode"),
                    ),
            )
            .child(
                h_flex()
                    .items_stretch()
                    .gap_3()
                    .child(self.render_current(ThemeKind::Light, cx))
                    .child(self.render_current(ThemeKind::Dark, cx)),
            )
            .child(
                h_flex()
                    .min_w_0()
                    .gap_3()
                    .flex_wrap()
                    .child(
                        Button::new("import-theme")
                            .small()
                            .label("Import")
                            .on_click(|_, _, cx| import_theme(cx)),
                    )
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(&self.query)
                                .id("theme-search")
                                .small()
                                .aria_label("Search themes"),
                        ),
                    )
                    .child(
                        h_flex().gap_1().children(
                            [
                                (Filter::All, "All"),
                                (Filter::Light, "Light"),
                                (Filter::Dark, "Dark"),
                                (Filter::Custom, "Custom"),
                            ]
                            .into_iter()
                            .enumerate()
                            .map(|(ix, (filter, label))| {
                                Button::new(("theme-filter", ix))
                                    .small()
                                    .ghost()
                                    .selected(self.filter == filter)
                                    .label(label)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.filter = filter;
                                        this.list.remeasure();
                                        cx.notify();
                                    }))
                            }),
                        ),
                    ),
            )
            .children((!empty).then(|| {
                list(self.list.clone(), move |ix, _, cx| {
                    page.update(cx, |page, cx| match rows.get(ix) {
                        Some(FamilyRow::Heading(title)) => div()
                            .py_2()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(*title)
                            .into_any_element(),
                        Some(FamilyRow::Family(id)) => {
                            cx.global::<ThemeLibrary>().family(*id).map_or_else(
                                || div().into_any_element(),
                                |family| page.render_family(family, cx).into_any_element(),
                            )
                        }
                        None => div().into_any_element(),
                    })
                })
                .w_full()
                .flex_1()
                .min_h_0()
            }))
            .children(empty.then(|| {
                div()
                    .py_4()
                    .text_color(cx.theme().muted_foreground)
                    .child("No themes match your search")
            }))
    }
}

fn open_editor(id: u64, variant: Option<u64>, window: &mut Window, cx: &mut App) {
    if let Some(view) = cx
        .try_global::<crate::app::AppState>()
        .and_then(|state| state.view.clone())
    {
        view.update(cx, |view, cx| {
            view.open_theme_editor_for(id, variant, window, cx);
        });
    }
}

fn delete_family(id: u64, window: &mut Window, cx: &mut App) {
    match cx.global_mut::<ThemeLibrary>().delete_family(id) {
        Ok(deleted) => {
            themes::refresh_current(cx);
            SettingsView::init_theme_options(cx);
            show_undo(id, deleted, window, cx);
        }
        Err(error) => {
            window.push_notification(notifications::settings_save_failed("theme", &error), cx);
        }
    }
}

pub fn show_undo(family_id: u64, deleted: DeletedTheme, window: &mut Window, cx: &mut App) {
    struct ThemeDeletion;
    let notification_id = rand::random::<u64>();
    let deleted = std::rc::Rc::new(std::cell::RefCell::new(Some(deleted)));
    window.push_notification(
        Notification::new()
            .id1::<ThemeDeletion>(("theme-deletion", notification_id))
            .title("Theme deleted")
            .message("You can restore it with Undo")
            .action(move |_, _, _| {
                let deleted = deleted.clone();
                Button::new("undo-theme-delete")
                    .small()
                    .label("Undo")
                    .on_click(move |_, window, cx| {
                        let pending = deleted.borrow_mut().take();
                        if let Some(undo) = pending {
                            if let Err(error) =
                                cx.global_mut::<ThemeLibrary>().undo_delete(undo.clone())
                            {
                                *deleted.borrow_mut() = Some(undo);
                                window.push_notification(
                                    notifications::settings_save_failed("theme", &error),
                                    cx,
                                );
                            } else {
                                themes::refresh_current(cx);
                                SettingsView::init_theme_options(cx);
                                if let Some(view) = cx
                                    .try_global::<crate::app::AppState>()
                                    .and_then(|state| state.view.clone())
                                {
                                    let editor =
                                        view.read(cx).tabs.theme_editor_for(family_id, cx).cloned();
                                    if let Some(editor) = editor {
                                        editor.update(cx, |editor, cx| {
                                            editor.refresh_variants(window, cx);
                                        });
                                    }
                                }
                                window.remove_notification1::<ThemeDeletion>(
                                    ("theme-deletion", notification_id),
                                    cx,
                                );
                                cx.refresh_windows();
                            }
                        }
                    })
            }),
        cx,
    );
}

fn import_theme(cx: &App) {
    let receiver = cx.prompt_for_paths(PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: Some("Import theme".into()),
    });
    cx.spawn(async move |cx| {
        if let Ok(Ok(Some(paths))) = receiver.await
            && let Some(path) = paths.first()
        {
            let path = path.clone();
            cx.update(|cx| dialogs::import_family(path, cx));
        }
    })
    .detach();
}

fn export_family(id: u64, cx: &App) {
    let Some(name) = cx
        .global::<ThemeLibrary>()
        .family(id)
        .map(|family| family.name().to_owned())
    else {
        return;
    };
    let receiver = cx.prompt_for_new_path(
        &std::env::current_dir().unwrap_or_default(),
        Some(&format!("{name}.json")),
    );
    cx.spawn(async move |cx| {
        if let Ok(Ok(Some(path))) = receiver.await {
            cx.update(|cx| {
                if let Err(error) = cx.global::<ThemeLibrary>().export(id, &path) {
                    notifications::push_window_notification(
                        cx,
                        notifications::settings_save_failed("theme export", &error),
                    );
                }
            });
        }
    })
    .detach();
}

pub mod dialogs;
