//! Theme selection in its own modal: one card per family, previewed in its own colors.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use gpui_kit::component::{
    ActiveTheme as _, Icon, IconName, Selectable as _, Sizable as _, Theme, WindowExt as _,
    button::{Button, ButtonVariants as _, Toggle, ToggleGroup, ToggleVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu as _, PopupMenuItem},
    scroll::ScrollableElement as _,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, FontWeight, Hsla, InteractiveElement as _,
    IntoElement, ParentElement, PathPromptOptions, Render, ScrollHandle, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, px, rems,
};

use crate::app::{
    AppState,
    fonts::FontCatalog,
    settings::{self, ThemeKind, ThemePreference},
    themes::{self, DeletedTheme, ResolvedAppearance, ThemeFamily, ThemeLibrary, ThemeVariant},
};
use crate::ui::{icons::DatalithIcon, notifications, themes as ui_themes};

/// Appearance modes in display order; each also filters the variants on offer.
const PREFERENCES: [ThemePreference; 3] = [
    ThemePreference::System,
    ThemePreference::Light,
    ThemePreference::Dark,
];

const CARD_WIDTH: f32 = 16.5;
const PREVIEW_HEIGHT: f32 = 6.5;
const CARD_HEIGHT: f32 = 11.75;

const fn preference_label(preference: ThemePreference) -> &'static str {
    match preference {
        ThemePreference::System => "System",
        ThemePreference::Light => "Light",
        ThemePreference::Dark => "Dark",
    }
}

fn preference_icon(preference: ThemePreference) -> Icon {
    match preference {
        ThemePreference::System => Icon::new(DatalithIcon::Contrast),
        ThemePreference::Light => Icon::new(IconName::Sun),
        ThemePreference::Dark => Icon::new(IconName::Moon),
    }
}

fn kind_icon(kind: ThemeKind) -> Icon {
    match kind {
        ThemeKind::Light => Icon::new(IconName::Sun),
        ThemeKind::Dark => Icon::new(IconName::Moon),
    }
}

const fn kind_name(kind: ThemeKind) -> &'static str {
    match kind {
        ThemeKind::Light => "light",
        ThemeKind::Dark => "dark",
    }
}

/// Whether the appearance mode offers this variant: System offers both kinds.
fn offered(preference: ThemePreference, variant: &ThemeVariant) -> bool {
    match preference {
        ThemePreference::System => true,
        ThemePreference::Light => !variant.mode().is_dark(),
        ThemePreference::Dark => variant.mode().is_dark(),
    }
}

/// The kinds whose current theme the appearance mode shows and lets you choose.
const fn offered_kinds(preference: ThemePreference) -> &'static [ThemeKind] {
    match preference {
        ThemePreference::System => &ThemeKind::ALL,
        ThemePreference::Light => &[ThemeKind::Light],
        ThemePreference::Dark => &[ThemeKind::Dark],
    }
}

/// A variant's short name in its family: its suffix, or its mode for a single-variant family.
fn variant_label(family: &ThemeFamily, variant: &ThemeVariant) -> SharedString {
    family
        .suffix(variant.id())
        .filter(|suffix| !suffix.is_empty())
        .map_or_else(
            || {
                if variant.mode().is_dark() {
                    "Dark".into()
                } else {
                    "Light".into()
                }
            },
            |suffix| suffix.to_owned().into(),
        )
}

pub(super) struct ThemePage {
    query: Entity<InputState>,
    /// The variant each family card previews, chosen with its variant buttons.
    shown: HashMap<u64, u64>,
    // Resolving a variant is costly, so cache it until the library changes.
    appearances: RefCell<HashMap<u64, Option<Rc<ResolvedAppearance>>>>,
    scroll: ScrollHandle,
    _library_subscription: Subscription,
    _query_subscription: Subscription,
}

impl ThemePage {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("Search themes"));
        let query_subscription = cx.subscribe_in(&query, window, |_, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        let library_subscription = cx.observe_global::<ThemeLibrary>(|this, cx| {
            this.appearances.get_mut().clear();
            cx.notify();
        });
        Self {
            query,
            shown: HashMap::new(),
            appearances: RefCell::default(),
            scroll: ScrollHandle::default(),
            _library_subscription: library_subscription,
            _query_subscription: query_subscription,
        }
    }

    fn appearance(&self, id: u64, cx: &App) -> Option<Rc<ResolvedAppearance>> {
        self.appearances
            .borrow_mut()
            .entry(id)
            .or_insert_with(|| {
                cx.global::<ThemeLibrary>()
                    .resolved(id, cx.global::<FontCatalog>())
                    .ok()
                    .map(Rc::new)
            })
            .clone()
    }

    fn matches(family: &ThemeFamily, query: &str) -> bool {
        query.is_empty()
            || family.name().to_lowercase().contains(query)
            || family
                .variants()
                .iter()
                .any(|variant| variant.name().to_lowercase().contains(query))
    }

    /// The variant a card previews: the one picked on the card, else a current one, else the first.
    fn shown_variant<'a>(
        &self,
        family: &ThemeFamily,
        variants: &[&'a ThemeVariant],
        cx: &App,
    ) -> Option<&'a ThemeVariant> {
        let library = cx.global::<ThemeLibrary>();
        let is_current = |variant: &ThemeVariant| {
            library.current(ThemeKind::from(variant.mode())) == variant.name()
        };
        self.shown
            .get(&family.id())
            .and_then(|id| variants.iter().find(|variant| variant.id() == *id))
            .or_else(|| variants.iter().find(|variant| is_current(variant)))
            .or_else(|| variants.first())
            .copied()
    }
}

impl ThemePage {
    fn render_header(preference: ThemePreference, cx: &Context<Self>) -> impl IntoElement {
        h_flex()
            .w_full()
            .flex_wrap()
            .justify_between()
            .gap_4()
            .child(
                h_flex()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("Appearance"),
                    )
                    .child(
                        ToggleGroup::new("theme-appearance")
                            .segmented()
                            .outline()
                            .small()
                            .children(PREFERENCES.iter().enumerate().map(|(ix, &item)| {
                                Toggle::new(("theme-appearance", ix))
                                    .gap_1p5()
                                    .px_2()
                                    .icon(preference_icon(item))
                                    .label(preference_label(item))
                                    .checked(item == preference)
                            }))
                            .on_click(move |checked, _, cx| {
                                if let Some(item) =
                                    PREFERENCES.iter().zip(checked).find_map(|(item, on)| {
                                        (*on && *item != preference).then_some(*item)
                                    })
                                {
                                    ui_themes::change_mode(item, cx);
                                }
                            }),
                    ),
            )
            .child(
                h_flex().gap_4().flex_wrap().children(
                    offered_kinds(preference)
                        .iter()
                        .map(|kind| Self::render_current(*kind, cx)),
                ),
            )
    }

    fn render_current(kind: ThemeKind, cx: &Context<Self>) -> impl IntoElement {
        let library = cx.global::<ThemeLibrary>();
        let name = library.current(kind).to_owned();
        let appearance = library.variant(&name).and_then(|variant| {
            library
                .resolved(variant.id(), cx.global::<FontCatalog>())
                .ok()
        });
        h_flex()
            .id(("current-theme", usize::from(kind == ThemeKind::Dark)))
            .gap_2()
            .text_sm()
            .aria_label(format!("Current {} theme: {name}", kind_name(kind)))
            .child(
                kind_icon(kind)
                    .small()
                    .text_color(cx.theme().muted_foreground),
            )
            .children(appearance.map(|appearance| {
                let theme = appearance.theme();
                div()
                    .px_1()
                    .rounded_sm()
                    .border_1()
                    .border_color(theme.primary)
                    .bg(theme.background)
                    .text_color(theme.foreground)
                    .text_xs()
                    .font_family(appearance.fonts().headings().clone())
                    .child("Aa")
            }))
            .child(name)
    }

    fn render_section_heading(title: &'static str, cx: &Context<Self>) -> gpui_kit::Div {
        h_flex().w_full().justify_between().pt_4().pb_2().child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(cx.theme().foreground)
                .child(title),
        )
    }

    fn render_card(
        &self,
        family: &ThemeFamily,
        preference: ThemePreference,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let variants: Vec<_> = family
            .variants()
            .iter()
            .filter(|variant| offered(preference, variant))
            .collect();
        let shown = self.shown_variant(family, &variants, cx)?;
        let id = family.id();
        let name = family.name().to_owned();
        let count = if variants.len() == family.variants().len() {
            format!("{} variants", family.variants().len())
        } else {
            format!("{} / {} variants", variants.len(), family.variants().len())
        };
        Some(
            v_flex()
                .id(("theme-family", id))
                .w(rems(CARD_WIDTH))
                .h(rems(CARD_HEIGHT))
                .p_2()
                .gap_2()
                .rounded_lg()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().background)
                .children(self.render_preview(family, shown, cx))
                .child(
                    h_flex()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .child(name.clone()),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(count),
                        ),
                )
                .child(
                    h_flex()
                        .items_start()
                        .gap_1()
                        .child(h_flex().flex_1().min_w_0().flex_wrap().gap_1().children(
                            variants.iter().map(|variant| {
                                let variant_id = variant.id();
                                let label = variant_label(family, variant);
                                let active = variant_id == shown.id();
                                Button::new(("theme-variant", variant_id))
                                    .xsmall()
                                    .ghost()
                                    .selected(active)
                                    .when(active, |button| button.text_color(cx.theme().primary))
                                    .label(label.clone())
                                    .accessibility_label(format!("Preview {label}"))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.shown.insert(id, variant_id);
                                        cx.notify();
                                    }))
                            }),
                        ))
                        .child(Self::render_family_action(family, &name)),
                )
                .into_any_element(),
        )
    }

    /// A miniature window drawn in the variant's own colors and fonts. Clicking it
    /// makes the variant the current theme of its kind.
    fn render_preview(
        &self,
        family: &ThemeFamily,
        variant: &ThemeVariant,
        cx: &Context<Self>,
    ) -> Option<impl IntoElement> {
        let appearance = self.appearance(variant.id(), cx)?;
        let id = variant.id();
        let kind = ThemeKind::from(variant.mode());
        let current = cx.global::<ThemeLibrary>().current(kind) == variant.name();
        let label = variant_label(family, variant);
        let group: SharedString = format!("theme-preview-{id}").into();
        let ring = cx.theme().ring;
        Some(
            div()
                .id(("theme-preview", id))
                .group(group.clone())
                .relative()
                .p(px(2.))
                .rounded_lg()
                .border_2()
                .border_color(if current {
                    cx.theme().primary
                } else {
                    gpui_kit::transparent_black()
                })
                .cursor_pointer()
                .tab_index(0)
                .focus_visible(move |style| style.border_color(ring))
                .role(gpui_kit::Role::RadioButton)
                .aria_toggled(if current {
                    gpui_kit::accesskit::Toggled::True
                } else {
                    gpui_kit::accesskit::Toggled::False
                })
                .aria_label(if current {
                    format!("{}, current {} theme", variant.name(), kind_name(kind))
                } else {
                    format!("Use {} as the {} theme", variant.name(), kind_name(kind))
                })
                .on_click(move |_, _, cx| {
                    if !current {
                        ui_themes::set_current(id, kind, cx);
                    }
                })
                .child(miniature(&appearance))
                .child(if current {
                    h_flex()
                        .absolute()
                        .top_2()
                        .right_2()
                        .gap_1()
                        .px_1p5()
                        .rounded_md()
                        .bg(cx.theme().primary)
                        .text_color(cx.theme().primary_foreground)
                        .text_xs()
                        .child(Icon::new(IconName::Check).xsmall())
                        .child(if kind == ThemeKind::Dark {
                            "Dark"
                        } else {
                            "Light"
                        })
                        .into_any_element()
                } else {
                    div()
                        .absolute()
                        .inset(px(2.))
                        .rounded_md()
                        .flex()
                        .items_center()
                        .justify_center()
                        .px_2()
                        .text_xs()
                        .text_center()
                        .bg(cx.theme().background.opacity(0.85))
                        .text_color(cx.theme().foreground)
                        .opacity(0.)
                        .group_hover(group, |style| style.opacity(1.))
                        .child(format!("Use {label} as {} theme", kind_name(kind)))
                        .into_any_element()
                }),
        )
    }

    fn render_family_action(family: &ThemeFamily, name: &str) -> AnyElement {
        let id = family.id();
        if !family.is_custom() {
            return Button::new(("customize-theme", id))
                .xsmall()
                .ghost()
                .icon(Icon::new(DatalithIcon::Pen))
                .tooltip("Customize")
                .accessibility_label(format!("Customize {name}"))
                .on_click(move |_, window, cx| customize_bundled(id, window, cx))
                .into_any_element();
        }
        Button::new(("theme-actions", id))
            .xsmall()
            .ghost()
            .icon(IconName::Ellipsis)
            .tooltip("Actions")
            .accessibility_label(format!("Actions for {name}"))
            .dropdown_menu(move |menu, _, _| {
                menu.item(
                    PopupMenuItem::new("Customize")
                        .on_click(move |_, window, cx| open_editor(id, window, cx)),
                )
                .item(PopupMenuItem::new("Export").on_click(move |_, _, cx| export_family(id, cx)))
                .separator()
                .item(
                    PopupMenuItem::element(|_, cx| {
                        div().text_color(cx.theme().danger).child("Delete")
                    })
                    .on_click(move |_, window, cx| delete_family(id, window, cx)),
                )
            })
            .into_any_element()
    }

    fn render_create_custom() -> impl IntoElement {
        Button::new("create-theme")
            .outline()
            .w(rems(CARD_WIDTH))
            .h(rems(CARD_HEIGHT))
            .border_dashed()
            .child(
                v_flex()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .child(Icon::new(IconName::Plus))
                    .child("Create a theme"),
            )
            .on_click(|_, window, cx| create_theme(window, cx))
    }

    fn render_cards(
        &self,
        custom: bool,
        preference: ThemePreference,
        query: &str,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        cx.global::<ThemeLibrary>()
            .families()
            .filter(|family| family.is_custom() == custom && Self::matches(family, query))
            .filter_map(|family| self.render_card(family, preference, cx))
            .collect()
    }

    fn render_cards_section(cards: Vec<AnyElement>, empty: impl IntoElement) -> AnyElement {
        if cards.is_empty() {
            return empty.into_any_element();
        }
        h_flex()
            .w_full()
            .flex_wrap()
            .items_start()
            .gap_4()
            .children(cards)
            .into_any_element()
    }

    fn render_no_match(text: &'static str, cx: &Context<Self>) -> impl IntoElement {
        div()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(text)
    }
}

/// A miniature Datalith window: title bar, sidebar with a selected row, and a
/// note in the variant's headings, reading and code fonts.
fn miniature(appearance: &ResolvedAppearance) -> impl IntoElement {
    let theme: &Theme = appearance.theme();
    let dot = |color: Hsla| div().size(px(5.)).rounded_full().bg(color);
    let bar = |color: Hsla, width: f32| {
        div()
            .h(px(4.))
            .w(gpui_kit::relative(width))
            .rounded_sm()
            .bg(color)
    };
    v_flex()
        .w_full()
        .h(rems(PREVIEW_HEIGHT))
        .overflow_hidden()
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .bg(theme.background)
        .text_color(theme.foreground)
        .text_xs()
        .child(
            div()
                .h(px(8.))
                .flex_none()
                .bg(theme.title_bar)
                .border_b_1()
                .border_color(theme.title_bar_border),
        )
        .child(
            h_flex()
                .flex_1()
                .min_h_0()
                .items_stretch()
                .child(
                    v_flex()
                        .w(gpui_kit::relative(0.24))
                        .flex_none()
                        .gap_1()
                        .px_1()
                        .py_1p5()
                        .bg(theme.sidebar)
                        .border_r_1()
                        .border_color(theme.sidebar_border)
                        .child(div().h(px(6.)).rounded_sm().bg(theme.sidebar_accent))
                        .child(bar(theme.sidebar_border, 0.7))
                        .child(bar(theme.sidebar_border, 0.55)),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_0p5()
                        .px_2()
                        .py_1p5()
                        .child(
                            div()
                                .truncate()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .font_family(appearance.fonts().headings().clone())
                                .child("Field notes"),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_color(theme.muted_foreground)
                                .font_family(appearance.fonts().reading().clone())
                                .child("Follow the thread…"),
                        )
                        .child(
                            div().flex().child(
                                div()
                                    .px_1()
                                    .rounded_sm()
                                    .bg(theme.muted)
                                    .font_family(appearance.fonts().code().clone())
                                    .child("build()"),
                            ),
                        )
                        .child(
                            h_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .px_1p5()
                                        .rounded_sm()
                                        .bg(theme.primary)
                                        .text_color(theme.primary_foreground)
                                        .font_family(appearance.fonts().interface().clone())
                                        .child("Open"),
                                )
                                .child(div().text_color(theme.link).underline().child("Atlas"))
                                .child(dot(theme.success))
                                .child(dot(theme.warning))
                                .child(dot(theme.danger)),
                        ),
                ),
        )
}

impl Render for ThemePage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let preference = settings::snapshot().theme_preference;
        let query = self.query.read(cx).value().to_lowercase();
        let custom = self.render_cards(true, preference, &query, cx);
        let bundled = self.render_cards(false, preference, &query, cx);
        let custom_section = h_flex()
            .w_full()
            .flex_wrap()
            .items_start()
            .gap_4()
            .child(Self::render_create_custom())
            .children(custom);
        v_flex()
            .id("theme-settings-page")
            .size_full()
            .min_w_0()
            .min_h_0()
            .gap_4()
            .child(Self::render_header(preference, cx))
            .child(
                Input::new(&self.query)
                    .id("theme-search")
                    .small()
                    .aria_label("Search themes"),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        v_flex()
                            .id("theme-cards")
                            .absolute()
                            .inset_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            // Keep focus rings inside the scroll clip.
                            .px_1()
                            .pb_4()
                            .child(
                                Self::render_section_heading("Custom", cx).child(
                                    Button::new("import-theme")
                                        .small()
                                        .label("Import…")
                                        .on_click(|_, _, cx| import_theme(cx)),
                                ),
                            )
                            .child(custom_section)
                            .child(Self::render_section_heading("Built-in", cx))
                            .child(Self::render_cards_section(
                                bundled,
                                Self::render_no_match("No themes match your search", cx),
                            )),
                    )
                    .vertical_scrollbar(&self.scroll),
            )
    }
}

/// Copy a built-in family into a custom "<name> - Copy" family and edit it.
fn customize_bundled(id: u64, window: &mut Window, cx: &mut App) {
    let library = cx.global_mut::<ThemeLibrary>();
    let Some(name) = library.family(id).map(|family| family.name().to_owned()) else {
        return;
    };
    let copy_name = library.suggested_copy_name(&name);
    match library.copy_family(id, &copy_name) {
        Ok(copy) => open_editor(copy, window, cx),
        Err(error) => {
            window.push_notification(notifications::settings_save_failed("theme", &error), cx);
        }
    }
}

fn open_editor(id: u64, window: &mut Window, cx: &mut App) {
    if let Some(view) = AppState::view(cx) {
        view.update(cx, |view, cx| view.open_theme_editor_for(id, window, cx));
    }
}

fn create_theme(window: &mut Window, cx: &mut App) {
    match cx.global_mut::<ThemeLibrary>().create_family() {
        Ok(id) => open_editor(id, window, cx),
        Err(error) => {
            window.push_notification(notifications::theme_create_failed(&error), cx);
        }
    }
}

/// Refresh the open editor for a family after the library changed it elsewhere.
pub fn refresh_theme_editor(family_id: u64, reload: bool, window: &mut Window, cx: &mut App) {
    let Some(editor) = AppState::view(cx)
        .and_then(|view| view.read(cx).tabs.theme_editor_for(family_id, cx).cloned())
    else {
        return;
    };
    editor.update(cx, |editor, cx| {
        if reload {
            editor.reload_variants(window, cx);
        } else {
            editor.refresh_variants(window, cx);
        }
    });
}

fn delete_family(id: u64, window: &mut Window, cx: &mut App) {
    match cx.global_mut::<ThemeLibrary>().delete_family(id) {
        Ok(deleted) => {
            themes::refresh_current(cx);
            show_undo(id, deleted, window, cx);
        }
        Err(error) => {
            window.push_notification(notifications::settings_save_failed("theme", &error), cx);
        }
    }
}

pub fn close_deleted_editor(family_id: u64, window: &Window, cx: &mut App) {
    if cx.global::<ThemeLibrary>().family(family_id).is_some() {
        return;
    }
    // Defer until the editor's current update has released its entity.
    window.defer(cx, move |window, cx| {
        if let Some(view) = AppState::view(cx) {
            view.update(cx, |view, cx| {
                view.close_theme_editor(family_id, window, cx);
            });
        }
    });
}

pub fn show_undo(family_id: u64, deleted: DeletedTheme, window: &mut Window, cx: &mut App) {
    close_deleted_editor(family_id, window, cx);
    let notification_id = rand::random::<u64>();
    let message = deleted.message();
    let deleted = std::rc::Rc::new(std::cell::RefCell::new(Some(deleted)));
    window.push_notification(
        notifications::theme_deletion(message, notification_id, move |window, cx| {
            let pending = deleted.borrow_mut().take();
            if let Some(undo) = pending {
                if let Err(error) = cx.global_mut::<ThemeLibrary>().undo_delete(undo.clone()) {
                    *deleted.borrow_mut() = Some(undo);
                    window.push_notification(
                        notifications::settings_save_failed("theme", &error),
                        cx,
                    );
                } else {
                    themes::refresh_current(cx);
                    refresh_theme_editor(family_id, false, window, cx);
                    notifications::remove_theme_deletion(notification_id, window, cx);
                    cx.refresh_windows();
                }
            }
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
            cx.update(|cx| {
                if let Err(error) = cx.global_mut::<ThemeLibrary>().import(&path) {
                    notifications::push_window_notification(
                        cx,
                        notifications::settings_save_failed("theme import", &error),
                    );
                }
            });
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
