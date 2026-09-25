//! Theme management within Settings. Family and variant identity comes from the library.

use std::collections::HashSet;

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, Selectable as _, Sizable as _, WindowExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu as _, PopupMenuItem},
    notification::Notification,
    setting::{SettingGroup, SettingItem},
    v_flex,
};
use gpui_kit::{
    App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ListAlignment,
    ListState, ParentElement, PathPromptOptions, Pixels, Render, Styled as _, Subscription, Window,
    div, list, px, rems,
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

pub(super) struct ThemePage {
    query: Entity<InputState>,
    filter: Filter,
    expanded: HashSet<u64>,
    rows: Vec<FamilyRow>,
    list: ListState,
    rem_size: Pixels,
    _query_subscription: Subscription,
}

impl ThemePage {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("Search themes"));
        let subscription = cx.subscribe_in(&query, window, |_, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        Self {
            query,
            filter: Filter::All,
            expanded: HashSet::new(),
            rows: Vec::new(),
            list: ListState::new(0, ListAlignment::Top, px(100.)),
            rem_size: window.rem_size(),
            _query_subscription: subscription,
        }
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

    fn render_mode() -> impl IntoElement {
        let current = settings::snapshot().theme_preference;
        h_flex().gap_2().flex_wrap().children(
            [
                (ThemePreference::Light, "Light"),
                (ThemePreference::Dark, "Dark"),
                (ThemePreference::System, "System"),
            ]
            .into_iter()
            .enumerate()
            .map(|(ix, (preference, label))| {
                Button::new(("theme-appearance", ix))
                    .small()
                    .label(label)
                    .selected(current == preference)
                    .on_click(move |_, _, cx| ui_themes::change_mode(preference, cx))
            }),
        )
    }

    fn render_current(kind: ThemeKind, cx: &Context<Self>) -> impl IntoElement {
        let library = cx.global::<ThemeLibrary>();
        let name = library.current(kind);
        let resolved = library.variant(name).and_then(|variant| {
            library
                .resolved(variant.id(), cx.global::<FontCatalog>())
                .ok()
        });
        let mode = match kind {
            ThemeKind::Light => "Light",
            ThemeKind::Dark => "Dark",
        };
        let swatches = resolved.as_ref().map(|appearance| {
            h_flex().gap_1().children(
                [
                    appearance.theme().background,
                    appearance.theme().primary,
                    appearance.theme().border,
                ]
                .into_iter()
                .enumerate()
                .map(|(index, color)| {
                    div()
                        .id(format!("current-theme-swatch-{mode}-{index}"))
                        .size_4()
                        .rounded_sm()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(color)
                }),
            )
        });
        h_flex()
            .id(format!("current-theme-{mode}"))
            .w_full()
            .min_w_0()
            .justify_between()
            .gap_3()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                v_flex()
                    .min_w_0()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("Current {mode}")),
                    )
                    .child(div().truncate().child(name.to_owned())),
            )
            .children(swatches)
    }

    fn render_variant(
        variant: &crate::app::themes::ThemeVariant,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let id = variant.id();
        let kind = ThemeKind::from(variant.mode());
        let current = cx.global::<ThemeLibrary>().current(kind) == variant.name();
        let name = variant.name().to_owned();
        let fonts = FontRole::ALL
            .into_iter()
            .filter_map(|role| {
                variant.document().font(role).map(|font| {
                    format!(
                        "{}: {font}",
                        match role {
                            FontRole::Interface => "Interface",
                            FontRole::Reading => "Reading",
                            FontRole::Headings => "Headings",
                            FontRole::Code => "Code",
                        }
                    )
                })
            })
            .collect::<Vec<_>>()
            .join(" · ");
        let label = match kind {
            ThemeKind::Light => "Set as Light theme",
            ThemeKind::Dark => "Set as Dark theme",
        };
        h_flex()
            .id(("theme-variant", id))
            .items_start()
            .w_full()
            .min_w_0()
            .gap_3()
            .py_2()
            .pl_4()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(div().w_40().flex_none().child(if current {
                Button::new(("current-variant", id))
                    .small()
                    .ghost()
                    .disabled(true)
                    .label("Current ✓")
            } else {
                Button::new(("set-current", id))
                    .small()
                    .ghost()
                    .label(label)
                    .on_click(move |_, _, cx| ui_themes::set_current(id, kind, cx))
            }))
            .child(
                v_flex()
                    .min_w_0()
                    .gap_1()
                    .child(
                        h_flex().gap_2().child(div().child(name)).child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(match kind {
                                    ThemeKind::Light => "Light",
                                    ThemeKind::Dark => "Dark",
                                }),
                        ),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(if fonts.is_empty() {
                                "Datalith defaults".to_owned()
                            } else {
                                fonts
                            }),
                    ),
            )
    }

    fn render_family(&self, family: &ThemeFamily, cx: &Context<Self>) -> impl IntoElement {
        let id = family.id();
        let expanded = self.expanded.contains(&id);
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
                    .child(
                        Button::new(("expand-theme", id))
                            .small()
                            .ghost()
                            .icon(if expanded {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            })
                            .tooltip(if expanded {
                                "Collapse variants"
                            } else {
                                "Expand variants"
                            })
                            .accessibility_label(format!(
                                "{} {name}",
                                if expanded { "Collapse" } else { "Expand" }
                            ))
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.toggle_family(id, cx)),
                            ),
                    )
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
                            .label("Edit")
                            .on_click(move |_, window, cx| open_editor(id, None, window, cx))
                    } else {
                        Button::new(("copy-theme", id))
                            .small()
                            .label("Copy & edit")
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
            .children(expanded.then(|| {
                v_flex().children(
                    family
                        .variants()
                        .iter()
                        .filter(|variant| match self.filter {
                            Filter::Light => {
                                variant.mode() == gpui_kit::component::ThemeMode::Light
                            }
                            Filter::Dark => variant.mode() == gpui_kit::component::ThemeMode::Dark,
                            Filter::All | Filter::Custom => true,
                        })
                        .map(|variant| Self::render_variant(variant, cx)),
                )
            }))
    }

    fn toggle_family(&mut self, id: u64, cx: &mut Context<Self>) {
        if !self.expanded.insert(id) {
            self.expanded.remove(&id);
        }
        if let Some(ix) = self
            .rows
            .iter()
            .position(|row| *row == FamilyRow::Family(id))
        {
            self.list.remeasure_items(ix..ix.saturating_add(1));
        }
        cx.notify();
    }
}

impl SettingsView {
    pub(super) fn theme_page_group(&self) -> SettingGroup {
        let page = self.theme_page.clone();
        SettingGroup::new().items(vec![SettingItem::render(move |_, _, _| {
            page.clone().into_any_element()
        })])
    }
}

impl ThemePage {
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
            .w_full()
            .gap_4()
            .child(
                v_flex()
                    .gap_2()
                    .child("Appearance")
                    .child(Self::render_mode()),
            )
            .child(
                v_flex()
                    .gap_1()
                    .child(Self::render_current(ThemeKind::Light, cx))
                    .child(Self::render_current(ThemeKind::Dark, cx)),
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
                .h(rems(32.))
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
