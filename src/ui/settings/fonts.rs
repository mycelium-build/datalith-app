use gpui_kit::component::{
    ActiveTheme, IndexPath, Sizable, Theme,
    searchable_list::{SearchableListItem, SearchableVec},
    select::{Select, SelectEvent, SelectState},
    setting::{RenderOptions, SettingField, SettingGroup, SettingItem},
    v_flex,
};
use gpui_kit::{
    App, AppContext, Axis, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, Styled, Subscription, Window, div, rems,
};

use super::SettingsView;
use crate::app::{
    fonts::{self, FontCatalog},
    settings::FontRole,
};
use crate::ui::notifications;

#[derive(Clone)]
struct FontOption {
    family: SharedString,
    label: SharedString,
    unavailable: bool,
}

impl SearchableListItem for FontOption {
    type Value = SharedString;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &SharedString {
        &self.family
    }

    fn disabled(&self) -> bool {
        self.unavailable
    }
}

type FontPickerState = SelectState<SearchableVec<FontOption>>;

pub(super) struct FontPicker {
    role: FontRole,
    state: Entity<FontPickerState>,
    default_label: SharedString,
    _subscriptions: Vec<Subscription>,
}

impl FontPicker {
    pub(super) fn new(role: FontRole, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let catalog = cx.global::<FontCatalog>();
        let selected = catalog.selected(role).unwrap_or_default();
        let default_label = default_label(role, cx);
        let options = font_options(role, default_label.clone(), catalog);
        let selected_ix = options
            .iter()
            .position(|option| option.family == selected)
            .unwrap_or(0);
        let state = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(options),
                Some(IndexPath::default().row(selected_ix)),
                window,
                cx,
            )
            .searchable(true)
        });
        let change = cx.subscribe_in(&state, window, |this, _, event, window, cx| {
            let SelectEvent::Confirm(Some(value)) = event else {
                return;
            };
            let family = (!value.is_empty()).then(|| value.to_string());
            if cx.global::<FontCatalog>().selected(this.role) == family.as_deref() {
                return;
            }
            if let Err(error) = fonts::select(this.role, family, cx) {
                this.sync_selection(window, cx);
                notifications::push_window_notification(
                    cx,
                    notifications::settings_save_failed("font", &error),
                );
            }
            cx.notify();
        });
        let sync = cx.observe_global_in::<FontCatalog>(window, |this, window, cx| {
            this.sync_selection(window, cx);
            cx.notify();
        });
        let theme = cx.observe_global_in::<Theme>(window, |this, window, cx| {
            this.sync_selection(window, cx);
            cx.notify();
        });
        Self {
            role,
            state,
            default_label,
            _subscriptions: vec![change, sync, theme],
        }
    }

    fn sync_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let default_label = default_label(self.role, cx);
        let options = (default_label != self.default_label)
            .then(|| font_options(self.role, default_label.clone(), cx.global::<FontCatalog>()));
        self.default_label = default_label;
        let selected: SharedString = cx
            .global::<FontCatalog>()
            .selected(self.role)
            .unwrap_or_default()
            .to_owned()
            .into();
        if options.is_some() || self.state.read(cx).selected_value() != Some(&selected) {
            self.state.update(cx, |state, cx| {
                if let Some(options) = options {
                    state.set_items(SearchableVec::new(options), window, cx);
                }
                state.set_selected_value(&selected, window, cx);
                cx.notify();
            });
        }
    }
}

impl Render for FontPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let catalog = cx.global::<FontCatalog>();
        let unavailable = catalog
            .selected(self.role)
            .is_some_and(|name| !catalog.contains(name));
        let family = fonts::family(self.role, cx);
        v_flex()
            .w_full()
            .min_w_0()
            .gap_2()
            .child(
                div().id(role_id(self.role)).w_full().child(
                    Select::new(&self.state)
                        .accessibility_label(match self.role {
                            FontRole::Interface => "Interface font",
                            FontRole::Reading => "Reading font",
                            FontRole::Headings => "Heading font",
                            FontRole::Code => "Code and editing font",
                        })
                        .small()
                        .w_full()
                        .menu_width(rems(22.))
                        .search_placeholder("Search fonts…")
                        .empty(|_, cx| {
                            div()
                                .p_3()
                                .text_color(cx.theme().muted_foreground)
                                .child("No fonts found")
                        }),
                ),
            )
            .child(
                div()
                    .text_sm()
                    .font_family(family.clone())
                    .child(match self.role {
                        FontRole::Interface => "Datalith · Files · Settings · 0123456789",
                        FontRole::Reading => "The quick brown fox · À bientôt · 0123456789",
                        FontRole::Headings => "A heading for your notes",
                        FontRole::Code => "let answer = 42; // 0O 1Il {} []",
                    }),
            )
            .children(unavailable.then(|| {
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("Font unavailable on this device. Using {family}."))
            }))
    }
}

impl SettingsView {
    pub(super) fn fonts_group(&self) -> SettingGroup {
        SettingGroup::new()
            .title("Fonts")
            .items(self.font_pickers.iter().map(|(role, picker)| {
                let picker = picker.clone();
                let (label, description) = match role {
                    FontRole::Interface => ("Interface", "Menus, tabs, sidebars and controls."),
                    FontRole::Reading => ("Reading", "Body text in Markdown previews."),
                    FontRole::Headings => ("Headings", "Titles and headings in Markdown previews."),
                    FontRole::Code => (
                        "Code and editing",
                        "Text editors and code in Markdown previews.",
                    ),
                };
                SettingItem::new(
                    label,
                    SettingField::element(move |_: &RenderOptions, _: &mut Window, _: &mut App| {
                        picker.clone()
                    }),
                )
                .description(description)
                .layout(Axis::Vertical)
            }))
    }
}

fn default_label(role: FontRole, cx: &App) -> SharedString {
    let family = fonts::default_family(role, cx);
    // GPUI's virtual macOS family refers to San Francisco.
    let name = if cfg!(target_os = "macos") && family == ".SystemUIFont" {
        "SF Pro"
    } else {
        family.as_str()
    };
    format!("Theme ({name})").into()
}

fn font_options(
    role: FontRole,
    default_label: SharedString,
    catalog: &FontCatalog,
) -> Vec<FontOption> {
    let mut options = vec![FontOption {
        family: SharedString::default(),
        label: default_label,
        unavailable: false,
    }];
    options.extend(catalog.families().iter().map(|family| FontOption {
        family: family.clone(),
        label: family.clone(),
        unavailable: false,
    }));
    if let Some(selected) = catalog
        .selected(role)
        .filter(|name| !catalog.contains(name))
    {
        options.push(FontOption {
            family: selected.to_owned().into(),
            label: format!("{selected} (unavailable)").into(),
            unavailable: true,
        });
    }
    options
}

const fn role_id(role: FontRole) -> &'static str {
    match role {
        FontRole::Interface => "font-interface",
        FontRole::Reading => "font-reading",
        FontRole::Headings => "font-headings",
        FontRole::Code => "font-code",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::component::Root;
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{Focusable as _, TestAppContext, px, size};

    #[test]
    fn default_label_tracks_theme_changes_without_changing_the_preference() {
        let mut cx = TestAppContext::single();
        cx.update(|cx| {
            gpui_kit::init(cx);
            FontCatalog::init(cx);
            fonts::use_theme_fonts(cx).unwrap();
        });
        let mut picker = None;
        let handle = cx.open_window(size(px(480.), px(360.)), |window, cx| {
            let view = cx.new(|cx| FontPicker::new(FontRole::Reading, window, cx));
            picker = Some(view.clone());
            Root::new(view, window, cx)
        });
        let picker = picker.unwrap();
        let state = cx.update(|cx| picker.read(cx).state.clone());
        for family in ["Arial", "Helvetica"] {
            cx.update(|cx| {
                let theme = Theme::global_mut(cx);
                std::rc::Rc::make_mut(&mut theme.light_theme).font_family = Some(family.into());
                std::rc::Rc::make_mut(&mut theme.dark_theme).font_family = Some(family.into());
                fonts::apply(cx);
            });
            cx.run_until_parked();
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                assert_eq!(
                    window.find(("select", state.entity_id())).value(),
                    Some(format!("Theme ({family})").as_str())
                );
                assert_eq!(fonts::family(FontRole::Reading, cx).as_str(), family);
                assert_eq!(cx.global::<FontCatalog>().selected(FontRole::Reading), None);
            })
            .unwrap();
        }
    }

    #[test]
    fn selecting_a_filtered_font_updates_the_preview_theme_and_saved_preference() {
        let mut cx = TestAppContext::single();
        cx.update(|cx| {
            gpui_kit::init(cx);
            FontCatalog::init(cx);
            fonts::use_theme_fonts(cx).unwrap();
        });
        let default_family = cx.update(|cx| cx.theme().font_family.clone());
        let mut picker = None;
        let handle = cx.open_window(size(px(480.), px(360.)), |window, cx| {
            let view = cx.new(|cx| FontPicker::new(FontRole::Interface, window, cx));
            picker = Some(view.clone());
            Root::new(view, window, cx)
        });
        let picker = picker.unwrap();
        let state = cx.update(|cx| picker.read(cx).state.clone());
        for (query, selected, expected_family) in [
            ("Arial", "Arial", "Arial"),
            ("Helvetica", "Helvetica", "Helvetica"),
            ("Theme", "", default_family.as_str()),
        ] {
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                if query == "Helvetica" {
                    window.click(("select", state.entity_id()), cx);
                } else {
                    window.focus(&state.focus_handle(cx), cx);
                    window.press("enter", cx);
                }
            })
            .unwrap();
            cx.run_until_parked();
            cx.update_window(handle.into(), |_, window, cx| {
                window.press("secondary-a", cx);
                window.input(query, cx);
            })
            .unwrap();
            cx.run_until_parked();
            cx.update_window(handle.into(), |_, window, cx| {
                window.press("down", cx);
                window.press("enter", cx);
            })
            .unwrap();
            cx.run_until_parked();
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                assert_eq!(
                    state.read(cx).selected_value().map(SharedString::as_str),
                    Some(selected)
                );
                assert_eq!(
                    fonts::family(FontRole::Interface, cx).as_str(),
                    expected_family
                );
                assert_eq!(cx.theme().font_family.as_str(), expected_family);
                assert_eq!(
                    crate::app::settings::snapshot()
                        .fonts
                        .family(FontRole::Interface),
                    (!selected.is_empty()).then_some(selected)
                );
                assert!(state.focus_handle(cx).is_focused(window));
            })
            .unwrap();
        }
    }

    #[test]
    fn escape_closes_the_font_menu_before_the_settings_overlay() {
        let mut cx = TestAppContext::single();
        cx.update(|cx| {
            gpui_kit::init(cx);
            fonts::load_embedded_fonts(cx);
            FontCatalog::init(cx);
            SettingsView::init_theme_options(cx);
            cx.set_global(crate::app::AppState::default());
        });
        let mut view = None;
        let handle = cx.open_window(size(px(1000.), px(800.)), |window, cx| {
            let app = cx.new(|cx| crate::ui::DatalithView::new(false, vec![], window, cx));
            app.update(cx, |view, _| {
                view.startup_driver = gpui_kit::Task::ready(());
                view.startup = None;
                view.settings.open();
            });
            view = Some(app.clone());
            Root::new(app, window, cx)
        });
        let view = view.unwrap();
        let picker = cx.update(|cx| {
            view.read(cx)
                .settings
                .font_pickers
                .first()
                .unwrap()
                .1
                .clone()
        });
        let state = cx.update(|cx| picker.read(cx).state.clone());
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.focus(&state.focus_handle(cx), cx);
            window.press("enter", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.press("escape", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(
                view.read(cx).settings.open,
                "Escape must only close the font menu"
            );
            assert!(state.focus_handle(cx).is_focused(window));
            window.press("escape", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| assert!(!view.read(cx).settings.open));
    }

    #[test]
    fn font_search_cancels_without_changing_the_selection() {
        let mut cx = TestAppContext::single();
        cx.update(|cx| {
            gpui_kit::init(cx);
            fonts::load_embedded_fonts(cx);
            FontCatalog::init(cx);
            fonts::use_theme_fonts(cx).unwrap();
        });
        let mut picker = None;
        let handle = cx.open_window(size(px(480.), px(360.)), |window, cx| {
            let view = cx.new(|cx| FontPicker::new(FontRole::Interface, window, cx));
            picker = Some(view.clone());
            Root::new(view, window, cx)
        });
        let picker = picker.unwrap();
        let state = cx.update(|cx| picker.read(cx).state.clone());
        let selected = cx.update(|cx| state.read(cx).selected_value().cloned());
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.focus(&state.focus_handle(cx), cx);
            window.press("enter", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.input("no-such-font-12345", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(state.read(cx).selected_value().cloned(), selected);
        });
        cx.update_window(handle.into(), |_, window, cx| {
            window.press("escape", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(state.focus_handle(cx).is_focused(window));
            assert_eq!(state.read(cx).selected_value().cloned(), selected);
        })
        .unwrap();
    }
}
