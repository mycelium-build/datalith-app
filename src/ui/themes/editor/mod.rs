//! A retained theme draft with live preview and explicit save/discard transitions.

mod render;
#[cfg(test)]
mod tests;

use gpui_kit::component::{
    ActiveTheme as _, Colorize as _, Disableable as _, IndexPath, Sizable as _,
    button::{Button, ButtonVariants as _},
    color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState},
    h_flex,
    input::{Input, InputEvent, InputState},
    searchable_list::{SearchableListItem, SearchableVec},
    select::{Select, SelectEvent, SelectState},
    v_flex,
};
use gpui_kit::{
    App, AppContext as _, Context, DismissEvent, Entity, EventEmitter, FocusHandle,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div, rems,
};

use crate::app::{
    fonts::{self, FontCatalog},
    settings::FontRole,
    themes::{self, ThemeDocument, ThemeLibrary},
};

#[derive(Clone)]
struct Choice {
    value: SharedString,
    label: SharedString,
    unavailable: bool,
}
impl Choice {
    fn new(value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            unavailable: false,
        }
    }
}
impl SearchableListItem for Choice {
    type Value = SharedString;
    fn title(&self) -> SharedString {
        self.label.clone()
    }
    fn value(&self) -> &SharedString {
        &self.value
    }
    fn disabled(&self) -> bool {
        self.unavailable
    }
}
type Choices = SelectState<SearchableVec<Choice>>;

#[derive(Clone)]
enum Pending {
    Close,
    Change(super::ThemeChange),
    New,
}

pub struct ThemeEditor {
    draft: ThemeDocument,
    dirty: bool,
    name: Entity<InputState>,
    selector: Entity<Choices>,
    fonts: Vec<(FontRole, Entity<Choices>)>,
    color_token: Entity<Choices>,
    color_hex: Entity<InputState>,
    color_picker: Entity<ColorPickerState>,
    pending: Option<Pending>,
    error: Option<String>,
    color_error: Option<String>,
    focus: FocusHandle,
    subscriptions: Vec<Subscription>,
}

impl EventEmitter<DismissEvent> for ThemeEditor {}

impl gpui_kit::Focusable for ThemeEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl ThemeEditor {
    pub(crate) fn request_change(
        &mut self,
        change: super::ThemeChange,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request(Pending::Change(change), window, cx);
    }

    pub(crate) fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.request(Pending::Close, window, cx);
    }

    pub(crate) const fn has_unsaved_changes(&self) -> bool {
        self.dirty || self.color_error.is_some()
    }

    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        if cx.try_global::<ThemeLibrary>().is_none() {
            ThemeLibrary::init(cx);
        }
        let draft = themes::active_document(cx);
        let name = cx.new(|cx| InputState::new(window, cx).default_value(save_name(&draft, cx)));
        let selector = choices(theme_choices(cx), draft.name(), window, cx);
        let fonts = FontRole::ALL
            .into_iter()
            .map(|role| {
                (
                    role,
                    choices(
                        font_choices(draft.font(role), cx),
                        draft.font(role).unwrap_or_default(),
                        window,
                        cx,
                    ),
                )
            })
            .collect::<Vec<_>>();
        let tokens = draft
            .colors()
            .unwrap_or_default()
            .keys()
            .map(|token| Choice::new(token.clone(), token.replace('.', " · ")))
            .collect();
        let color_token = choices(tokens, "background", window, cx);
        let color_hex = cx.new(|cx| InputState::new(window, cx).placeholder("Mode default"));
        let color_picker = cx.new(|cx| ColorPickerState::new(window, cx));
        let subscriptions = vec![
            cx.subscribe_in(&name, window, |this, _, event, _, cx| {
                if matches!(event, InputEvent::Change)
                    && this.name.read(cx).value().as_str() != save_name(&this.draft, cx)
                {
                    this.dirty = true;
                    this.error = None;
                    cx.notify();
                }
            }),
            cx.subscribe_in(&selector, window, |this, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(name)) = event
                    && name.as_str() != this.draft.name()
                {
                    this.request_change(
                        super::ThemeChange::Select {
                            name: name.to_string(),
                            activate: true,
                        },
                        window,
                        cx,
                    );
                }
            }),
            cx.subscribe_in(&color_token, window, |this, _, event, window, cx| {
                if matches!(event, SelectEvent::Confirm(_)) {
                    this.sync_color(window, cx);
                    cx.notify();
                }
            }),
            cx.subscribe_in(&color_hex, window, |this, _, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.change_color(window, cx);
                }
            }),
            cx.subscribe_in(&color_picker, window, |this, _, event, window, cx| {
                let ColorPickerEvent::Change(color) = event;
                let value = color.map_or_else(String::new, |color| color.to_hex());
                this.color_hex
                    .update(cx, |input, cx| input.set_value(value, window, cx));
            }),
        ];
        let mut this = Self {
            draft,
            dirty: false,
            name,
            selector,
            fonts,
            color_token,
            color_hex,
            color_picker,
            pending: None,
            error: None,
            color_error: None,
            focus: cx.focus_handle(),
            subscriptions,
        };
        this.subscribe_fonts(window, cx);
        this.sync_color(window, cx);
        this.focus.focus(window, cx);
        this
    }

    fn subscribe_fonts(&mut self, window: &Window, cx: &mut Context<Self>) {
        for (role, state) in &self.fonts {
            let role = *role;
            self.subscriptions.push(cx.subscribe_in(
                state,
                window,
                move |this, _, event, _, cx| {
                    if let SelectEvent::Confirm(Some(value)) = event {
                        let family = (!value.is_empty()).then(|| value.to_string());
                        if this.draft.font(role) != family.as_deref() {
                            this.draft.set_font(role, family);
                            this.changed(cx);
                        }
                    }
                },
            ));
        }
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.dirty = true;
        self.error = None;
        themes::preview(&self.draft, cx);
        cx.notify();
    }

    fn selected_token(&self, cx: &App) -> SharedString {
        self.color_token
            .read(cx)
            .selected_value()
            .cloned()
            .unwrap_or_else(|| "background".into())
    }

    fn sync_color(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let token = self.selected_token(cx);
        let value = self
            .draft
            .colors()
            .ok()
            .and_then(|colors| colors.get(token.as_str()).cloned().flatten());
        self.color_hex.update(cx, |input, cx| {
            input.set_value(value.clone().unwrap_or_default(), window, cx);
        });
        self.color_picker.update(cx, |picker, cx| {
            if let Some(color) =
                value.and_then(|value| gpui_kit::component::try_parse_color(&value).ok())
            {
                picker.set_value(color, window, cx);
            } else {
                picker.clear_value(window, cx);
            }
        });
        self.color_error = None;
    }

    fn change_color(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let token = self.selected_token(cx);
        let input = self.color_hex.read(cx).value();
        let value = (!input.trim().is_empty()).then(|| input.trim().to_owned());
        if self
            .draft
            .colors()
            .ok()
            .and_then(|colors| colors.get(token.as_str()).cloned().flatten())
            == value
        {
            if self.color_error.take().is_some() {
                cx.notify();
            }
            return;
        }
        if self.draft.set_color(&token, value).is_ok() {
            self.color_error = None;
            self.sync_color(window, cx);
            self.changed(cx);
        } else {
            self.color_error = Some("Enter a valid color, such as #6750A4.".into());
            cx.notify();
        }
    }

    fn request(&mut self, pending: Pending, window: &mut Window, cx: &mut Context<Self>) {
        // The select shows the applied theme until the leave decision is resolved.
        let name: SharedString = self.draft.name().to_owned().into();
        self.selector
            .update(cx, |state, cx| state.set_selected_value(&name, window, cx));
        if self.dirty || self.color_error.is_some() {
            self.pending = Some(pending);
            self.focus.focus(window, cx);
            cx.notify();
        } else {
            self.proceed(pending, window, cx);
        }
    }

    fn proceed(&mut self, pending: Pending, window: &mut Window, cx: &mut Context<Self>) {
        themes::discard_preview(cx);
        self.pending = None;
        match pending {
            Pending::Close => {
                cx.emit(DismissEvent);
            }
            Pending::Change(change) => {
                super::apply_change(change, cx);
                self.load_active(window, cx);
            }
            Pending::New => {
                self.load_active(window, cx);
                let name = copy_name(self.draft.name(), cx);
                self.name.update(cx, |input, cx| {
                    input.set_value(name, window, cx);
                    input.focus(window, cx);
                });
                self.dirty = true;
            }
        }
        cx.notify();
    }

    fn load_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.draft = themes::active_document(cx);
        self.dirty = false;
        self.error = None;
        let name = save_name(&self.draft, cx);
        self.name
            .update(cx, |input, cx| input.set_value(name, window, cx));
        self.selector.update(cx, |state, cx| {
            state.set_items(SearchableVec::new(theme_choices(cx)), window, cx);
            state.set_selected_value(&self.draft.name().to_owned().into(), window, cx);
        });
        for (role, state) in &self.fonts {
            state.update(cx, |state, cx| {
                state.set_items(
                    SearchableVec::new(font_choices(self.draft.font(*role), cx)),
                    window,
                    cx,
                );
                state.set_selected_value(
                    &self.draft.font(*role).unwrap_or_default().to_owned().into(),
                    window,
                    cx,
                );
            });
        }
        self.sync_color(window, cx);
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.color_error.is_some() {
            return false;
        }
        let mut document = self.draft.clone();
        document.set_name(&self.name.read(cx).value());
        let replacing = (document.name() == self.draft.name()
            && cx.global::<ThemeLibrary>().is_custom(self.draft.name()))
        .then(|| self.draft.name().to_owned());
        if let Err(error) = cx
            .global_mut::<ThemeLibrary>()
            .save(&document, replacing.as_deref())
        {
            self.error = Some(error.to_string());
            cx.notify();
            return false;
        }
        themes::discard_preview(cx);
        super::super::settings::SettingsView::init_theme_options(cx);
        if let Err(error) = themes::select(document.name(), true, cx) {
            self.load_active(window, cx);
            self.error = Some(format!(
                "{} was saved, but could not be applied: {error}. Select it to try again.",
                document.name()
            ));
            cx.notify();
            return false;
        }
        self.load_active(window, cx);
        cx.notify();
        true
    }
}

fn choices(
    items: Vec<Choice>,
    selected: &str,
    window: &mut Window,
    cx: &mut App,
) -> Entity<Choices> {
    let ix = items
        .iter()
        .position(|choice| choice.value == selected)
        .unwrap_or(0);
    cx.new(|cx| {
        SelectState::new(
            SearchableVec::new(items),
            Some(IndexPath::default().row(ix)),
            window,
            cx,
        )
        .searchable(true)
    })
}

fn theme_choices(cx: &App) -> Vec<Choice> {
    let library = cx.global::<ThemeLibrary>();
    [
        gpui_kit::component::ThemeMode::Light,
        gpui_kit::component::ThemeMode::Dark,
    ]
    .into_iter()
    .flat_map(|mode| {
        library.options(mode).into_iter().map(move |(name, _)| {
            Choice::new(
                name.clone(),
                format!("{name} ({})", if mode.is_dark() { "Dark" } else { "Light" }),
            )
        })
    })
    .collect()
}

fn font_choices(selected: Option<&str>, cx: &App) -> Vec<Choice> {
    let catalog = cx.global::<FontCatalog>();
    let mut choices = vec![Choice::new("", "Application default")];
    choices.extend(
        catalog
            .families()
            .iter()
            .map(|family| Choice::new(family.clone(), family.clone())),
    );
    if let Some(family) = selected.filter(|family| !catalog.contains(family)) {
        choices.push(Choice {
            value: family.to_owned().into(),
            label: format!("{family} (unavailable)").into(),
            unavailable: true,
        });
    }
    choices
}

fn save_name(document: &ThemeDocument, cx: &App) -> String {
    if cx.global::<ThemeLibrary>().is_custom(document.name()) {
        document.name().to_owned()
    } else {
        copy_name(document.name(), cx)
    }
}

fn copy_name(name: &str, cx: &App) -> String {
    let library = cx.global::<ThemeLibrary>();
    let mut candidate = format!("{name} Custom");
    let mut number = 2u32;
    while library.get(&candidate).is_some() {
        candidate = format!("{name} Custom {number}");
        number = number.saturating_add(1);
    }
    candidate
}
