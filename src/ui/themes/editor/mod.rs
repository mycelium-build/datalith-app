//! Retained controls for one custom theme family. Valid edits go straight to the library.

mod colors;
mod render;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap};

use gpui_kit::component::{
    Colorize as _, IndexPath,
    color_picker::{ColorPickerEvent, ColorPickerState},
    input::{InputEvent, InputState},
    searchable_list::{SearchableListItem, SearchableVec},
    select::{SelectEvent, SelectState},
};
use gpui_kit::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, ListAlignment, ListState,
    Render, ScrollHandle, SharedString, Subscription, Window,
};

use crate::app::{
    fonts::FontCatalog,
    settings::FontRole,
    themes::{self, ThemeLibrary},
};

#[derive(Clone)]
struct Choice {
    value: SharedString,
    label: SharedString,
}
impl Choice {
    fn new(value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
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
}
type Choices = SelectState<SearchableVec<Choice>>;

struct ColorRow {
    controls: Option<ColorControls>,
    valid: Option<String>,
    display: String,
    error: Option<String>,
}

// Allocate the native input, picker, sliders and their subscriptions only after
// a row is edited. The row keeps its buffer when another row becomes active.
struct ColorControls {
    input: Entity<InputState>,
    picker: Entity<ColorPickerState>,
    _subscriptions: Vec<Subscription>,
}

struct VariantControls {
    fonts: [Entity<Choices>; 4],
    colors: BTreeMap<String, ColorRow>,
    subscriptions: Vec<Subscription>,
}

pub struct ThemeEditor {
    family_id: u64,
    edited: u64,
    selector: Entity<Choices>,
    variants: HashMap<u64, VariantControls>,
    category: &'static str,
    show_preview: bool,
    active_color: Option<(u64, String)>,
    preview: Option<themes::ResolvedAppearance>,
    preview_pane: Entity<super::preview::ThemePreview>,
    scroll: ScrollHandle,
    color_list: ListState,
    visible_colors: Vec<String>,
    color_query: Entity<InputState>,
    color_group: Entity<Choices>,
    _group_subscription: Subscription,
    rem_size: gpui_kit::Pixels,
    _query_subscription: Subscription,
    error: Option<String>,
    focus: FocusHandle,
    _selector_subscription: Subscription,
}

impl Focusable for ThemeEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl ThemeEditor {
    pub(crate) const fn family_id(&self) -> u64 {
        self.family_id
    }
    pub(crate) fn family_name(&self, cx: &App) -> String {
        cx.global::<ThemeLibrary>()
            .family(self.family_id)
            .map_or_else(|| "Theme".into(), |family| family.name().into())
    }
    pub(crate) fn new(
        family_id: u64,
        requested: Option<u64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let variants = cx
            .global::<ThemeLibrary>()
            .family(family_id)
            .map(|family| family.variants().to_vec())
            .unwrap_or_default();
        let edited = requested
            .filter(|id| variants.iter().any(|v| v.id() == *id))
            .or_else(|| variants.first().map(crate::app::themes::ThemeVariant::id))
            .unwrap_or_default();
        let selector = make_choices(
            variants
                .iter()
                .map(|v| {
                    Choice::new(
                        v.id().to_string(),
                        cx.global::<ThemeLibrary>()
                            .family(family_id)
                            .map_or_else(|| v.name().to_owned(), |f| render::variant_label(f, v)),
                    )
                })
                .collect(),
            &edited.to_string(),
            window,
            cx,
        );
        let selector_subscription =
            cx.subscribe_in(&selector, window, |this, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(value)) = event
                    && let Ok(id) = value.parse::<u64>()
                {
                    this.select(id, window, cx);
                    this.reset_property_scroll();
                }
            });
        let color_query =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search all colors…"));
        let query_subscription = cx.subscribe(&color_query, |this, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                this.refresh_color_list(cx);
                cx.notify();
            }
        });
        let color_group = make_choices(
            colors::GROUPS
                .iter()
                .map(|group| Choice::new(*group, *group))
                .collect(),
            "All areas",
            window,
            cx,
        );
        let group_subscription = cx.subscribe(&color_group, |this, _, event, cx| {
            if matches!(event, SelectEvent::Confirm(_)) {
                this.refresh_color_list(cx);
                cx.notify();
            }
        });
        let mut this = Self {
            family_id,
            edited,
            selector,
            variants: HashMap::new(),
            category: "Colors",
            show_preview: false,
            active_color: None,
            preview: None,
            preview_pane: cx.new(|cx| super::preview::ThemePreview::new(window, cx)),
            scroll: ScrollHandle::default(),
            color_list: ListState::new(0, ListAlignment::Top, gpui_kit::px(0.)),
            visible_colors: Vec::new(),
            color_query,
            color_group,
            _group_subscription: group_subscription,
            rem_size: window.rem_size(),
            _query_subscription: query_subscription,
            error: None,
            focus: cx.focus_handle(),
            _selector_subscription: selector_subscription,
        };
        this.refresh_preview(cx);
        this.mount_variant(edited, window, cx);
        this.refresh_color_list(cx);
        this
    }

    pub(crate) fn select(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if !self.variants.contains_key(&id) {
            self.mount_variant(id, window, cx);
        }
        self.set_edited(id, window, cx);
    }

    // Controls and preview always follow the same edited variant.
    fn set_edited(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.edited == id || !self.variants.contains_key(&id) {
            return;
        }
        self.edited = id;
        self.refresh_preview(cx);
        self.refresh_color_list(cx);
        self.selector.update(cx, |selector, cx| {
            selector.set_selected_value(&id.to_string().into(), window, cx);
        });
        cx.notify();
    }

    fn refresh_preview(&mut self, cx: &mut App) {
        self.preview = cx
            .global::<ThemeLibrary>()
            .resolved(self.edited, cx.global::<FontCatalog>())
            .ok();
        self.preview_pane
            .update(cx, |pane, cx| pane.set_appearance(self.preview.clone(), cx));
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Retained inputs, pickers and subscriptions are initialized together for one variant"
    )]
    fn mount_variant(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some((document, resolved)) = ({
            let library = cx.global::<ThemeLibrary>();
            library.family(self.family_id).and_then(|family| {
                family
                    .variants()
                    .iter()
                    .find(|v| v.id() == id)
                    .map(|variant| {
                        (
                            variant.document().clone(),
                            library.resolved(id, cx.global::<FontCatalog>()).ok(),
                        )
                    })
            })
        }) else {
            return;
        };
        let mut colors = document.colors().unwrap_or_default();
        let highlight =
            serde_json::to_value(document.config().highlight.clone().unwrap_or_default())
                .unwrap_or_default();
        if let Some(map) = highlight.as_object() {
            for (key, value) in map {
                if key == "syntax" {
                    if let Some(syntax) = value.as_object() {
                        for (key, style) in syntax {
                            colors.insert(
                                format!("highlight:syntax.{key}"),
                                style
                                    .get("color")
                                    .and_then(serde_json::Value::as_str)
                                    .map(str::to_owned),
                            );
                        }
                    }
                } else {
                    colors.insert(
                        format!("highlight:{key}"),
                        value.as_str().map(str::to_owned),
                    );
                }
            }
        }
        let fonts = FontRole::ALL.map(|role| {
            let value = document.font(role).unwrap_or_default();
            make_choices(font_choices(&document, role, cx), value, window, cx)
        });
        let mut rows = BTreeMap::new();
        for (token, valid) in colors {
            let display = valid
                .clone()
                .or_else(|| {
                    resolved
                        .as_ref()
                        .and_then(|appearance| resolved_color(appearance, &token))
                })
                .unwrap_or_default();
            rows.insert(
                token,
                ColorRow {
                    controls: None,
                    valid,
                    display,
                    error: None,
                },
            );
        }
        self.variants.insert(
            id,
            VariantControls {
                fonts,
                colors: rows,
                subscriptions: Vec::new(),
            },
        );
        let mut subscriptions = Vec::new();
        let font_states = self
            .variants
            .get(&id)
            .map(|controls| {
                FontRole::ALL
                    .into_iter()
                    .zip(controls.fonts.iter().cloned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for (role, state) in font_states {
            subscriptions.push(cx.on_focus_in(
                &state.read(cx).focus_handle(cx),
                window,
                move |this, window, cx| this.select(id, window, cx),
            ));
            subscriptions.push(cx.subscribe_in(
                &state,
                window,
                move |this, _, event, window, cx| {
                    if let SelectEvent::Confirm(Some(value)) = event {
                        this.select(id, window, cx);
                        let value = (!value.is_empty()).then(|| value.to_string());
                        let result = cx.global_mut::<ThemeLibrary>().update_font(id, role, value);
                        this.handle_change(result, id, cx);
                        this.refresh_font_choices(id, window, cx);
                    }
                },
            ));
        }
        if let Some(controls) = self.variants.get_mut(&id) {
            controls.subscriptions = subscriptions;
        }
    }

    fn edit_color(
        &mut self,
        id: u64,
        token: &str,
        open_picker: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some((previous_id, previous_token)) = self.active_color.take() {
            self.blur_color(previous_id, &previous_token, window, cx);
        }
        self.select(id, window, cx);
        let Some(row) = self
            .variants
            .get_mut(&id)
            .and_then(|v| v.colors.get_mut(token))
        else {
            return;
        };
        if row.controls.is_none() {
            let input = cx.new(|cx| InputState::new(window, cx).default_value(row.display.clone()));
            let picker = cx.new(|cx| ColorPickerState::new(window, cx));
            if let Ok(color) = gpui_kit::component::try_parse_color(&row.display) {
                picker.update(cx, |picker, cx| picker.set_value(color, window, cx));
            }
            let token = token.to_owned();
            let key = token.clone();
            let input_subscription = cx.subscribe_in(
                &input,
                window,
                move |this, _, event, window, cx| match event {
                    InputEvent::Focus => this.select(id, window, cx),
                    InputEvent::Change => this.change_color(id, &key, window, cx),
                    InputEvent::Blur => this.blur_color(id, &key, window, cx),
                    InputEvent::PressEnter { .. } => {}
                },
            );
            let picker_subscription =
                cx.subscribe_in(&picker, window, move |this, _, event, window, cx| {
                    let ColorPickerEvent::Change(color) = event;
                    if let Some(color) = color {
                        this.select(id, window, cx);
                        let text = color.to_hex();
                        if let Some(controls) = this
                            .variants
                            .get(&id)
                            .and_then(|v| v.colors.get(&token))
                            .and_then(|row| row.controls.as_ref())
                        {
                            controls
                                .input
                                .update(cx, |input, cx| input.set_value(text, window, cx));
                        }
                        this.change_color(id, &token, window, cx);
                    }
                });
            row.controls = Some(ColorControls {
                input,
                picker,
                _subscriptions: vec![input_subscription, picker_subscription],
            });
        }
        if let Some(controls) = &row.controls {
            window.focus(&controls.input.focus_handle(cx), cx);
            if open_picker {
                controls
                    .picker
                    .update(cx, |picker, cx| picker.set_open(true, cx));
            }
        }
        self.active_color = Some((id, token.to_owned()));
        self.color_list.remeasure();
        cx.notify();
    }

    fn change_color(&mut self, id: u64, token: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self
            .variants
            .get_mut(&id)
            .and_then(|v| v.colors.get_mut(token))
        else {
            return;
        };
        let Some(controls) = &row.controls else {
            return;
        };
        let value = controls.input.read(cx).value();
        let parsed = gpui_kit::component::try_parse_color(&value);
        if parsed.is_err() {
            row.error = Some("Enter a valid color, such as #6750A4".into());
            self.color_list.remeasure();
            cx.notify();
            return;
        }
        if row.error.take().is_some() {
            self.color_list.remeasure();
        }
        if row.valid.as_deref() == Some(value.as_str()) {
            cx.notify();
            return;
        }
        let result = if let Some(token) = token.strip_prefix("highlight:") {
            cx.global_mut::<ThemeLibrary>()
                .update_highlight(id, token, Some(value.to_string()))
        } else {
            cx.global_mut::<ThemeLibrary>()
                .update_color(id, token, Some(value.to_string()))
        };
        if result.is_ok() {
            row.valid = Some(value.to_string());
            row.display = value.to_string();
            if let Ok(color) = parsed {
                controls
                    .picker
                    .update(cx, |picker, cx| picker.set_value(color, window, cx));
            }
        }
        self.handle_change(result, id, cx);
        self.refresh_inherited_colors(id, window, cx);
        self.color_list.remeasure();
    }

    fn blur_color(&mut self, id: u64, token: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(row) = self
            .variants
            .get_mut(&id)
            .and_then(|v| v.colors.get_mut(token))
            && row.error.take().is_some()
            && let Some(controls) = &row.controls
        {
            controls.input.update(cx, |input, cx| {
                input.set_value(row.display.clone(), window, cx);
            });
            self.color_list.remeasure();
            cx.notify();
        }
    }

    fn reset_color(&mut self, id: u64, token: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.select(id, window, cx);
        let result = if let Some(token) = token.strip_prefix("highlight:") {
            cx.global_mut::<ThemeLibrary>()
                .update_highlight(id, token, None)
        } else {
            cx.global_mut::<ThemeLibrary>()
                .update_color(id, token, None)
        };
        if result.is_ok() {
            let display = cx
                .global::<ThemeLibrary>()
                .resolved(id, cx.global::<FontCatalog>())
                .ok()
                .and_then(|appearance| resolved_color(&appearance, token))
                .unwrap_or_default();
            if let Some(row) = self
                .variants
                .get_mut(&id)
                .and_then(|v| v.colors.get_mut(token))
            {
                row.valid = None;
                Self::sync_color_display(row, display, window, cx);
            }
        }
        self.handle_change(result, id, cx);
        self.refresh_inherited_colors(id, window, cx);
        self.color_list.remeasure();
    }

    fn sync_color_display(
        row: &mut ColorRow,
        display: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        row.error = None;
        if let Some(controls) = &row.controls {
            controls
                .input
                .update(cx, |input, cx| input.set_value(display.clone(), window, cx));
            if let Ok(color) = gpui_kit::component::try_parse_color(&display) {
                controls
                    .picker
                    .update(cx, |picker, cx| picker.set_value(color, window, cx));
            }
        }
        row.display = display;
    }

    fn refresh_inherited_colors(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(appearance) = &self.preview
            && let Some(controls) = self.variants.get_mut(&id)
        {
            for (token, row) in &mut controls.colors {
                if row.valid.is_none() {
                    let display = resolved_color(appearance, token).unwrap_or_default();
                    Self::sync_color_display(row, display, window, cx);
                }
            }
        }
    }

    fn apply_fonts_to_all(&mut self, from: u64, window: &mut Window, cx: &mut Context<Self>) {
        let result = cx.global_mut::<ThemeLibrary>().apply_fonts_to_all(from);
        if result.is_ok() {
            let ids: Vec<_> = self.variants.keys().copied().collect();
            for id in ids {
                self.refresh_font_choices(id, window, cx);
            }
        }
        let applied = result.is_ok();
        self.handle_change(result, from, cx);
        if applied {
            themes::refresh_current(cx);
        }
    }

    fn refresh_font_choices(&self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(document) = cx
            .global::<ThemeLibrary>()
            .variant_by_id(id)
            .map(|variant| variant.document().clone())
        else {
            return;
        };
        if let Some(controls) = self.variants.get(&id) {
            for (role, state) in FontRole::ALL.into_iter().zip(&controls.fonts) {
                state.update(cx, |state, cx| {
                    state.set_items(
                        SearchableVec::new(font_choices(&document, role, cx)),
                        window,
                        cx,
                    );
                    state.set_selected_value(
                        &document.font(role).unwrap_or_default().to_owned().into(),
                        window,
                        cx,
                    );
                });
            }
        }
    }

    fn handle_change(&mut self, result: anyhow::Result<u64>, id: u64, cx: &mut Context<Self>) {
        match result {
            Ok(family_id) => {
                self.error = None;
                self.refresh_preview(cx);
                let name = cx
                    .global::<ThemeLibrary>()
                    .variant_by_id(id)
                    .map(|v| v.name().to_owned());
                if name.is_some_and(|name| {
                    [
                        crate::app::settings::ThemeKind::Light,
                        crate::app::settings::ThemeKind::Dark,
                    ]
                    .iter()
                    .any(|kind| cx.global::<ThemeLibrary>().current(*kind) == name)
                }) {
                    themes::refresh_current(cx);
                }
                ThemeLibrary::schedule_save(family_id, cx);
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        cx.notify();
    }

    fn add_variant(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let invalid: Vec<_> = self
            .variants
            .get(&self.edited)
            .into_iter()
            .flat_map(|variant| variant.colors.iter())
            .filter(|(_, row)| row.error.is_some())
            .map(|(token, _)| token.clone())
            .collect();
        for token in invalid {
            self.blur_color(self.edited, &token, window, cx);
        }
        let Some(family) = cx.global::<ThemeLibrary>().family(self.family_id) else {
            return;
        };
        if family.variants().len() == 1 {
            super::super::settings::theme::dialogs::name_variants(
                self.family_id,
                self.edited,
                window,
                cx,
            );
            return;
        }
        let suffix = cx.global::<ThemeLibrary>().next_suffix(self.family_id);
        if let Ok(suffix) = suffix {
            match cx.global_mut::<ThemeLibrary>().add_variant(
                self.family_id,
                self.edited,
                &suffix,
                None,
            ) {
                Ok(id) => {
                    self.mount_variant(id, window, cx);
                    self.refresh_variants(window, cx);
                    self.select(id, window, cx);
                    self.reset_property_scroll();
                    super::super::settings::theme::dialogs::rename_variant(
                        self.family_id,
                        id,
                        window,
                        cx,
                    );
                    ThemeLibrary::schedule_save(self.family_id, cx);
                }
                Err(error) => self.error = Some(error.to_string()),
            }
        }
        cx.notify();
    }

    fn change_variant_mode(
        &mut self,
        id: u64,
        mode: gpui_kit::component::ThemeMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select(id, window, cx);
        match cx.global_mut::<ThemeLibrary>().change_mode(id, mode) {
            Ok(()) => {
                self.refresh_preview(cx);
                self.refresh_inherited_colors(id, window, cx);
                ThemeLibrary::schedule_save(self.family_id, cx);
                themes::refresh_current(cx);
                crate::ui::settings::SettingsView::init_theme_options(cx);
                cx.notify();
            }
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
            }
        }
    }

    fn remove_variant(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        match cx.global_mut::<ThemeLibrary>().remove_variant(id) {
            Ok(deleted) => {
                self.variants.remove(&id);
                if self.edited == id
                    && let Some(first) = cx
                        .global::<ThemeLibrary>()
                        .family(self.family_id)
                        .and_then(|family| family.variants().first())
                {
                    self.edited = first.id();
                }
                self.refresh_variants(window, cx);
                themes::refresh_current(cx);
                crate::ui::settings::SettingsView::init_theme_options(cx);
                super::super::settings::theme::show_undo(self.family_id, deleted, window, cx);
                cx.notify();
            }
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
            }
        }
    }

    pub(crate) fn refresh_variants(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ids = cx
            .global::<ThemeLibrary>()
            .family(self.family_id)
            .map(|f| {
                f.variants()
                    .iter()
                    .map(crate::app::themes::ThemeVariant::id)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.variants.retain(|id, _| ids.contains(id));
        if !ids.contains(&self.edited) {
            self.edited = ids.first().copied().unwrap_or_default();
        }
        if !self.variants.contains_key(&self.edited) {
            self.mount_variant(self.edited, window, cx);
            self.reset_property_scroll();
        }
        let choices: Vec<Choice> = cx
            .global::<ThemeLibrary>()
            .family(self.family_id)
            .map(|f| {
                f.variants()
                    .iter()
                    .map(|v| Choice::new(v.id().to_string(), render::variant_label(f, v)))
                    .collect()
            })
            .unwrap_or_default();
        self.selector.update(cx, |selector, cx| {
            selector.set_items(SearchableVec::new(choices), window, cx);
            selector.set_selected_value(&self.edited.to_string().into(), window, cx);
        });
        self.refresh_preview(cx);
        self.refresh_color_list(cx);
        cx.notify();
    }

    fn refresh_color_list(&mut self, cx: &App) {
        let query = self.color_query.read(cx).value().trim().to_lowercase();
        let group = self
            .color_group
            .read(cx)
            .selected_value()
            .map_or("All areas", |value| value.as_str());
        self.visible_colors = self
            .variants
            .get(&self.edited)
            .map_or_else(Vec::new, |variant| {
                if self.category == "Advanced" {
                    variant
                        .colors
                        .keys()
                        .filter(|token| {
                            (group == "All areas" || colors::group(token) == group)
                                && (token.to_lowercase().contains(&query)
                                    || render::color_label(token).to_lowercase().contains(&query)
                                    || colors::description(token).to_lowercase().contains(&query))
                        })
                        .cloned()
                        .collect()
                } else {
                    render::ESSENTIAL_COLORS
                        .iter()
                        .filter(|(token, _, _)| variant.colors.contains_key(*token))
                        .map(|(token, _, _)| (*token).to_owned())
                        .collect()
                }
            });
        if self.category == "Advanced" {
            self.visible_colors.sort_by_cached_key(|token| {
                (
                    colors::GROUPS
                        .iter()
                        .position(|group| *group == colors::group(token))
                        .unwrap_or(usize::MAX),
                    token.clone(),
                )
            });
        }
        self.color_list.reset(self.visible_colors.len());
    }

    fn reset_property_scroll(&self) {
        self.color_list.scroll_to(gpui_kit::ListOffset::default());
        self.scroll
            .set_offset(gpui_kit::point(gpui_kit::px(0.), gpui_kit::px(0.)));
    }

    pub(crate) fn reload_variants(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.variants.clear();
        self.active_color = None;
        self.refresh_variants(window, cx);
        cx.notify();
    }
}

fn make_choices(
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

fn font_choices(document: &themes::ThemeDocument, role: FontRole, cx: &App) -> Vec<Choice> {
    let selected = document.font(role).unwrap_or_default();
    let catalog = cx.global::<FontCatalog>();
    let mut default = document.clone();
    default.set_font(role, None);
    let fallback = catalog
        .resolve_roles(&default)
        .into_iter()
        .nth(role.index())
        .unwrap_or_default();
    let mut choices = vec![Choice::new("", format!("Default ({fallback})"))];
    choices.extend(
        catalog
            .families()
            .iter()
            .map(|family| Choice::new(family.clone(), family.clone())),
    );
    if !selected.is_empty() && !catalog.contains(selected) {
        choices.push(Choice::new(
            selected.to_owned(),
            format!("{selected} (unavailable)"),
        ));
    }
    choices
}

fn resolved_color(
    appearance: &crate::app::themes::ResolvedAppearance,
    token: &str,
) -> Option<String> {
    appearance.color(token).map(|color| color.to_hex())
}
