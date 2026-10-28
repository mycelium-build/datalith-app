//! Retained controls for one custom theme family. Valid edits go straight to the library.

mod colors;
mod fonts;
mod regions;
mod render;
mod roles;
mod variants;

use std::{collections::BTreeMap, rc::Rc};

use conv::ConvUtil as _;

use gpui_kit::component::{
    Colorize as _, IndexPath,
    color_picker::{ColorPickerEvent, ColorPickerState},
    input::{InputEvent, InputState},
    searchable_list::{SearchableListItem, SearchableVec},
    select::{SelectEvent, SelectState},
};
use gpui_kit::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, Hsla, ListAlignment, ListState,
    Rgba, ScrollHandle, SharedString, Subscription, Window,
};

use crate::app::{
    fonts::FontCatalog,
    settings::FontRole,
    themes::{self, ResolvedAppearance, ThemeDocument, ThemeLibrary},
};

#[derive(Clone)]
struct Choice<T = SharedString> {
    value: T,
    label: SharedString,
}
impl<T> Choice<T> {
    fn new(value: impl Into<T>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
        }
    }
}
impl<T: Clone + PartialEq> SearchableListItem for Choice<T> {
    type Value = T;
    fn title(&self) -> SharedString {
        self.label.clone()
    }
    fn value(&self) -> &T {
        &self.value
    }
}
type Choices<T = SharedString> = SelectState<SearchableVec<Choice<T>>>;

struct ColorRow {
    controls: Option<ColorControls>,
    valid: Option<String>,
    display: String,
}

// Allocate the native picker and its subscriptions only when a row is edited.
struct ColorControls {
    picker: Entity<ColorPickerState>,
    _subscriptions: Vec<Subscription>,
}

type FontControl = Entity<Choices<Option<SharedString>>>;

struct FontControls {
    interface: FontControl,
    reading: FontControl,
    headings: FontControl,
    code: FontControl,
}

impl FontControls {
    fn iter(&self) -> impl Iterator<Item = (FontRole, &FontControl)> {
        [
            (FontRole::Interface, &self.interface),
            (FontRole::Reading, &self.reading),
            (FontRole::Headings, &self.headings),
            (FontRole::Code, &self.code),
        ]
        .into_iter()
    }
}

fn color_hex(color: Hsla) -> String {
    let rgba = Rgba::from(color);
    let channel = |value: f32| {
        (value.clamp(0., 1.) * 255.)
            .round()
            .approx_as::<u8>()
            .unwrap_or(0)
    };
    let rgb = format!(
        "#{:02x}{:02x}{:02x}",
        channel(rgba.r),
        channel(rgba.g),
        channel(rgba.b)
    );
    if rgba.a < 1. {
        format!("{rgb}{:02x}", channel(rgba.a))
    } else {
        rgb
    }
}

struct VariantControls {
    fonts: FontControls,
    colors: BTreeMap<String, ColorRow>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Clone, Copy)]
struct ColorOriginFilters {
    theme_defined: bool,
    component_default: bool,
}

impl ColorOriginFilters {
    const fn includes(self, origin: colors::ColorOrigin) -> bool {
        match origin {
            colors::ColorOrigin::ThemeDefined => self.theme_defined,
            colors::ColorOrigin::ComponentDefault => self.component_default,
        }
    }

    const fn set(&mut self, origin: colors::ColorOrigin, selected: bool) {
        match origin {
            colors::ColorOrigin::ThemeDefined => self.theme_defined = selected,
            colors::ColorOrigin::ComponentDefault => self.component_default = selected,
        }
    }

    const fn all_selected(self) -> bool {
        self.theme_defined && self.component_default
    }
}

impl Default for ColorOriginFilters {
    fn default() -> Self {
        Self {
            theme_defined: true,
            component_default: true,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Category {
    Colors,
    Fonts,
    AllColors,
}

impl Category {
    const ALL: [Self; 3] = [Self::Colors, Self::Fonts, Self::AllColors];

    const fn label(self) -> &'static str {
        match self {
            Self::Colors => "Colors",
            Self::Fonts => "Fonts",
            Self::AllColors => "All colors",
        }
    }

    /// The group filter's choices; the first one matches every color.
    const fn groups(self) -> &'static [&'static str] {
        match self {
            Self::AllColors => colors::FAMILIES,
            Self::Colors | Self::Fonts => colors::ESSENTIAL_GROUPS,
        }
    }

    fn all_groups(self) -> &'static str {
        self.groups().first().copied().unwrap_or_default()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RenameTarget {
    Family,
    Variant(u64),
}

struct RenameSession {
    target: RenameTarget,
    input: Entity<InputState>,
    _subscription: Subscription,
}

#[derive(Debug, PartialEq, Eq)]
enum PropertyRow {
    Color(String),
    GroupHeading(&'static str),
}

pub struct ThemeEditor {
    family_id: u64,
    edited: u64,
    controls: VariantControls,
    category: Category,
    show_preview: bool,
    /// The color token being edited in the `edited` variant.
    active_color: Option<String>,
    preview: Option<Rc<ResolvedAppearance>>,
    /// Every variant's resolved colors and fonts, shared by the miniatures.
    variant_miniatures: BTreeMap<u64, Entity<variants::VariantMiniature>>,
    preview_pane: Entity<super::preview::ThemePreview>,
    /// Created right after the editor, since they observe it.
    regions: Option<regions::EditorRegions>,
    scroll: ScrollHandle,
    color_list: ListState,
    /// The Colors category's header and cards.
    roles_list: ListState,
    property_rows: Vec<PropertyRow>,
    color_query: Entity<InputState>,
    color_group: Entity<Choices>,
    color_origins: ColorOriginFilters,
    _group_subscription: Subscription,
    rem_size: gpui_kit::Pixels,
    _query_subscription: Subscription,
    error: Option<String>,
    rename: Option<RenameSession>,
    focus: FocusHandle,
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
    pub(crate) fn try_new(
        family_id: u64,
        window: &mut Window,
        cx: &mut App,
    ) -> anyhow::Result<Entity<Self>> {
        let family = cx
            .global::<ThemeLibrary>()
            .family(family_id)
            .ok_or_else(|| anyhow::anyhow!("Theme family no longer exists"))?;
        let variant = family
            .variants()
            .first()
            .ok_or_else(|| anyhow::anyhow!("Theme family has no variants"))?;
        let edited = variant.id();
        let document = variant.document().clone();
        let editor = cx.new(|cx: &mut Context<Self>| {
            let color_query =
                cx.new(|cx| InputState::new(window, cx).placeholder("Search colors…"));
            let query_subscription = cx.subscribe(&color_query, |this, _, event, cx| {
                if matches!(event, InputEvent::Change) {
                    this.refresh_color_list(cx);
                    cx.notify();
                }
            });
            let color_group: Entity<Choices> = make_choices(
                group_choices(Category::Colors),
                &Category::Colors.all_groups().into(),
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
                controls: Self::variant_controls(edited, &document, window, cx),
                category: Category::Colors,
                show_preview: false,
                active_color: None,
                preview: None,
                variant_miniatures: BTreeMap::new(),
                preview_pane: cx.new(|cx| super::preview::ThemePreview::new(window, cx)),
                regions: None,
                scroll: ScrollHandle::default(),
                color_list: ListState::new(0, ListAlignment::Top, gpui_kit::px(0.)),
                roles_list: ListState::new(roles::ROLE_ITEMS, ListAlignment::Top, gpui_kit::px(0.)),
                property_rows: Vec::new(),
                color_query,
                color_group,
                color_origins: ColorOriginFilters::default(),
                _group_subscription: group_subscription,
                rem_size: window.rem_size(),
                _query_subscription: query_subscription,
                error: None,
                rename: None,
                focus: cx.focus_handle(),
            };
            this.refresh_variant_appearances(cx);
            this.refresh_preview(cx);
            this.refresh_color_list(cx);
            this
        });
        editor.update(cx, |editor, cx| {
            editor.regions = Some(regions::EditorRegions::new(&cx.entity(), cx));
        });
        Ok(editor)
    }

    /// Below this width the variants move above the controls and the
    /// preview moves behind a toggle.
    fn is_narrow(window: &Window) -> bool {
        window.viewport_size().width.as_f32() < window.rem_size().as_f32() * 80.
    }

    pub(crate) fn select(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_rename(window, cx);
        if self.edited == id {
            return;
        }
        let Some(variant) = cx
            .global::<ThemeLibrary>()
            .family(self.family_id)
            .and_then(|family| family.variants().iter().find(|variant| variant.id() == id))
        else {
            return;
        };
        let document = variant.document().clone();
        self.apply_variant(id, &document, window, cx);
    }

    fn apply_variant(
        &mut self,
        id: u64,
        document: &ThemeDocument,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let controls = Self::variant_controls(id, document, window, cx);
        let changed = self.edited != id;
        self.active_color = None;
        self.controls = controls;
        self.edited = id;
        self.refresh_preview(cx);
        self.refresh_color_list(cx);
        self.roles_list.remeasure();
        if changed {
            self.reset_property_scroll();
        }
        cx.notify();
    }

    fn set_category(&mut self, category: Category, window: &mut Window, cx: &mut Context<Self>) {
        if self.category == category {
            return;
        }
        self.category = category;
        self.color_group.update(cx, |group, cx| {
            group.set_items(SearchableVec::new(group_choices(category)), window, cx);
        });
        self.reset_color_filters(window, cx);
        self.reset_property_scroll();
        cx.notify();
    }

    fn reset_color_filters(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let all_groups = self.category.all_groups();
        self.color_group.update(cx, |group, cx| {
            group.set_selected_value(&all_groups.into(), window, cx);
        });
        self.color_query
            .update(cx, |query, cx| query.set_value("", window, cx));
        self.color_origins = ColorOriginFilters::default();
        self.refresh_color_list(cx);
    }

    fn is_active_color(&self, token: &str) -> bool {
        self.active_color.as_deref() == Some(token)
    }

    /// Close the open picker, if any, and return its token.
    fn close_active_picker(&mut self, cx: &mut Context<Self>) -> Option<String> {
        let token = self.active_color.take()?;
        if let Some(picker) = self
            .controls
            .colors
            .get(&token)
            .and_then(|row| row.controls.as_ref())
            .map(|controls| controls.picker.clone())
        {
            picker.update(cx, |picker, cx| picker.set_open(false, cx));
        }
        Some(token)
    }

    fn refresh_preview(&mut self, cx: &mut App) {
        self.preview = cx
            .global::<ThemeLibrary>()
            .resolved(self.edited, cx.global::<FontCatalog>())
            .ok()
            .map(Rc::new);
        if let Some(appearance) = self.preview.clone() {
            self.set_miniature(self.edited, appearance, cx);
        }
        self.preview_pane
            .update(cx, |pane, cx| pane.set_appearance(self.preview.clone(), cx));
    }

    /// Resolve every variant of the family, dropping removed ones.
    fn refresh_variant_appearances(&mut self, cx: &mut App) {
        let library = cx.global::<ThemeLibrary>();
        let catalog = cx.global::<FontCatalog>();
        let appearances: Vec<_> = library
            .family(self.family_id)
            .map(|family| {
                family
                    .variants()
                    .iter()
                    .filter_map(|variant| {
                        let id = variant.id();
                        library
                            .resolved(id, catalog)
                            .ok()
                            .map(|appearance| (id, Rc::new(appearance)))
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.variant_miniatures
            .retain(|id, _| appearances.iter().any(|(variant, _)| variant == id));
        for (id, appearance) in appearances {
            self.set_miniature(id, appearance, cx);
        }
    }

    fn set_miniature(&mut self, id: u64, appearance: Rc<ResolvedAppearance>, cx: &mut App) {
        if let Some(miniature) = self.variant_miniatures.get(&id) {
            miniature.update(cx, |miniature, cx| miniature.set_appearance(appearance, cx));
        } else {
            self.variant_miniatures
                .insert(id, cx.new(|_| variants::VariantMiniature::new(appearance)));
        }
    }

    fn variant_controls(
        id: u64,
        document: &ThemeDocument,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> VariantControls {
        let library = cx.global::<ThemeLibrary>();
        let resolved = library.resolved(id, cx.global::<FontCatalog>()).ok();
        let mut colors = document.colors().unwrap_or_default();
        colors.extend(themes::highlight_tokens(
            &document.config().highlight.clone().unwrap_or_default(),
        ));
        let mut font_control = |role| {
            let value = document.font(role).map(SharedString::from);
            make_choices(font_choices(document, role, cx), &value, window, cx)
        };
        let fonts = FontControls {
            interface: font_control(FontRole::Interface),
            reading: font_control(FontRole::Reading),
            headings: font_control(FontRole::Headings),
            code: font_control(FontRole::Code),
        };
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
                },
            );
        }
        let mut subscriptions = Vec::new();
        for (role, state) in fonts.iter() {
            subscriptions.push(cx.subscribe_in(
                state,
                window,
                move |this, _, event, window, cx| {
                    if let SelectEvent::Confirm(Some(value)) = event
                        && this.edited == id
                    {
                        let value = value.as_ref().map(ToString::to_string);
                        let result = cx.global_mut::<ThemeLibrary>().update_font(id, role, value);
                        this.handle_change(result, id, cx);
                        this.refresh_font_choices(id, window, cx);
                    }
                },
            ));
        }
        VariantControls {
            fonts,
            colors: rows,
            _subscriptions: subscriptions,
        }
    }

    fn edit_color(&mut self, id: u64, token: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.close_active_picker(cx);
        let Some(row) = self.controls.colors.get_mut(token) else {
            return;
        };
        if row.controls.is_none() {
            let picker = cx.new(|cx| ColorPickerState::new(window, cx));
            if let Ok(color) = gpui_kit::component::try_parse_color(&row.display) {
                picker.update(cx, |picker, cx| picker.set_value(color, window, cx));
            }
            let key = token.to_owned();
            let observer = cx.observe_in(&picker, window, move |this, picker, _, cx| {
                if !picker.read(cx).is_open() {
                    this.finish_color_edit(id, &key, cx);
                }
            });
            let key = token.to_owned();
            let subscription =
                cx.subscribe_in(&picker, window, move |this, _, event, window, cx| {
                    if let ColorPickerEvent::Change(Some(color)) = event
                        && this.edited == id
                    {
                        this.change_color(id, &key, *color, window, cx);
                    }
                });
            row.controls = Some(ColorControls {
                picker,
                _subscriptions: vec![subscription, observer],
            });
        }
        if let Some(controls) = &row.controls {
            window.focus(&controls.picker.focus_handle(cx), cx);
            controls
                .picker
                .update(cx, |picker, cx| picker.set_open(true, cx));
        }
        self.active_color = Some(token.to_owned());
        self.color_list.remeasure();
        cx.notify();
    }

    fn change_color(
        &mut self,
        id: u64,
        token: &str,
        color: Hsla,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = color_hex(color);
        let result = cx
            .global_mut::<ThemeLibrary>()
            .update_token(id, token, Some(value.clone()));
        if result.is_ok()
            && let Some(row) = self.controls.colors.get_mut(token)
        {
            row.valid = Some(value.clone());
            row.display = value;
        }
        self.handle_change(result, id, cx);
        self.refresh_inherited_colors(window, cx);
    }

    fn finish_color_edit(&mut self, id: u64, token: &str, cx: &mut Context<Self>) {
        if self.edited == id && self.is_active_color(token) {
            self.active_color = None;
            self.refresh_color_list(cx);
            self.color_list.remeasure();
            cx.notify();
        }
    }

    /// Buttons keep focus where it is, so an open name field never blurs
    /// when another control is clicked; commit it explicitly instead.
    fn finish_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(target) = self.rename.as_ref().map(|session| session.target) {
            self.commit_rename(target, window, cx);
        }
    }

    fn start_rename(
        &mut self,
        target: RenameTarget,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_rename(window, cx);
        let input = cx.new(|cx| InputState::new(window, cx).default_value(value));
        let subscription =
            cx.subscribe_in(
                &input,
                window,
                move |this, _, event, window, cx| match event {
                    InputEvent::Change => {
                        this.error = None;
                        cx.notify();
                    }
                    InputEvent::PressEnter { .. } | InputEvent::Blur => {
                        this.commit_rename(target, window, cx);
                    }
                    InputEvent::Focus => {}
                },
            );
        self.rename = Some(RenameSession {
            target,
            input: input.clone(),
            _subscription: subscription,
        });
        self.error = None;
        input.focus_handle(cx).focus(window, cx);
        window.dispatch_action(Box::new(gpui_kit::component::input::SelectAll), cx);
        cx.notify();
    }

    fn start_family_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = cx
            .global::<ThemeLibrary>()
            .family(self.family_id)
            .map(|family| family.name().to_owned())
        else {
            return;
        };
        self.start_rename(RenameTarget::Family, name, window, cx);
    }

    fn start_variant_rename(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        self.select(id, window, cx);
        let Some(name) = cx
            .global::<ThemeLibrary>()
            .family(self.family_id)
            .and_then(|family| family.suffix(id))
            .map(str::to_owned)
        else {
            return;
        };
        self.start_rename(RenameTarget::Variant(id), name, window, cx);
    }

    fn commit_rename(&mut self, target: RenameTarget, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = self
            .rename
            .as_ref()
            .filter(|session| session.target == target)
        else {
            return;
        };
        let value = session.input.read(cx).value().to_string();
        let result = match target {
            RenameTarget::Family => cx
                .global_mut::<ThemeLibrary>()
                .rename_family(self.family_id, &value),
            RenameTarget::Variant(id) => cx.global_mut::<ThemeLibrary>().rename_variant(id, &value),
        };
        match result {
            Ok(()) => {
                self.rename = None;
                self.error = None;
                themes::refresh_current(cx);
                self.refresh_variants(window, cx);
                self.focus.focus(window, cx);
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        cx.notify();
    }

    fn reset_color(&mut self, id: u64, token: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.select(id, window, cx);
        let result = cx
            .global_mut::<ThemeLibrary>()
            .update_token(id, token, None);
        if result.is_ok()
            && let Some(row) = self.controls.colors.get_mut(token)
        {
            row.valid = None;
        }
        self.handle_change(result, id, cx);
        self.refresh_inherited_colors(window, cx);
        if self.edited == id && self.is_active_color(token) {
            self.active_color = None;
        }
        self.refresh_color_list(cx);
        self.color_list.remeasure();
    }

    fn sync_color_display(
        row: &mut ColorRow,
        display: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(controls) = &row.controls
            && let Ok(color) = gpui_kit::component::try_parse_color(&display)
        {
            controls
                .picker
                .update(cx, |picker, cx| picker.set_value(color, window, cx));
        }
        row.display = display;
    }

    fn refresh_inherited_colors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(appearance) = &self.preview {
            for (token, row) in &mut self.controls.colors {
                if row.valid.is_none() {
                    let display = resolved_color(appearance, token).unwrap_or_default();
                    Self::sync_color_display(row, display, window, cx);
                }
            }
        }
    }

    fn apply_fonts_to_all(&mut self, from: u64, window: &mut Window, cx: &mut Context<Self>) {
        let result = cx.global_mut::<ThemeLibrary>().apply_fonts_to_all(from);
        let applied = result.is_ok();
        if applied {
            self.refresh_font_choices(from, window, cx);
        }
        self.handle_change(result, from, cx);
        if applied {
            self.refresh_variant_appearances(cx);
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
        for (role, state) in self.controls.fonts.iter() {
            state.update(cx, |state, cx| {
                state.set_items(
                    SearchableVec::new(font_choices(&document, role, cx)),
                    window,
                    cx,
                );
                state.set_selected_value(&document.font(role).map(SharedString::from), window, cx);
            });
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
                    crate::app::settings::ThemeKind::ALL
                        .into_iter()
                        .any(|kind| cx.global::<ThemeLibrary>().current(kind) == name)
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
        let library = cx.global_mut::<ThemeLibrary>();
        let Some(family) = library.family(self.family_id) else {
            super::super::settings::theme::close_deleted_editor(self.family_id, window, cx);
            return;
        };
        let first = (family.variants().len() == 1).then_some("Variant 1");
        let suffix = if first.is_some() {
            Ok("Variant 2".to_owned())
        } else {
            library.next_suffix(self.family_id)
        };
        let result = suffix
            .and_then(|suffix| library.add_variant(self.family_id, self.edited, &suffix, first));
        match result {
            Ok(id) => {
                self.refresh_variants(window, cx);
                themes::refresh_current(cx);
                self.start_variant_rename(id, window, cx);
                ThemeLibrary::schedule_save(self.family_id, cx);
            }
            Err(error) => self.error = Some(error.to_string()),
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
                self.refresh_inherited_colors(window, cx);
                self.refresh_color_list(cx);
                ThemeLibrary::schedule_save(self.family_id, cx);
                themes::refresh_current(cx);
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
                self.refresh_variants(window, cx);
                themes::refresh_current(cx);
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
        self.sync_variants(false, window, cx);
    }

    fn sync_variants(&mut self, reload: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh_variant_appearances(cx);
        let Some(family) = cx.global::<ThemeLibrary>().family(self.family_id) else {
            return;
        };
        let Some(variant) = family
            .variants()
            .iter()
            .find(|variant| variant.id() == self.edited)
            .or_else(|| family.variants().first())
        else {
            return;
        };
        if reload || variant.id() != self.edited {
            let id = variant.id();
            let document = variant.document().clone();
            self.apply_variant(id, &document, window, cx);
        } else {
            self.refresh_preview(cx);
            self.refresh_color_list(cx);
            cx.notify();
        }
    }

    fn refresh_color_list(&mut self, cx: &App) {
        let query = self.color_query.read(cx).value().trim().to_lowercase();
        let advanced = self.category == Category::AllColors;
        let all_groups = self.category.all_groups();
        let group = self
            .color_group
            .read(cx)
            .selected_value()
            .map_or(all_groups, |value| value.as_str());
        let origins = self.color_origins;
        let preview = self.preview.as_ref();
        let property_rows = {
            let variant = &self.controls;
            let mut visible = Vec::new();
            if advanced {
                for category in colors::GROUPS {
                    let tokens = variant
                        .colors
                        .keys()
                        .filter(|token| {
                            let active = self.is_active_color(token);
                            let origin = colors::origin(
                                preview
                                    .is_some_and(|appearance| appearance.color_is_defined(token)),
                            );
                            colors::group(token) == *category
                                && (group == all_groups || colors::family(token) == group)
                                && (origins.includes(origin) || active)
                                && (query.is_empty()
                                    || token.to_lowercase().contains(&query)
                                    || render::color_label(token).to_lowercase().contains(&query)
                                    || colors::description(token).to_lowercase().contains(&query))
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    if !tokens.is_empty() {
                        visible.push(PropertyRow::GroupHeading(category));
                        visible.extend(tokens.into_iter().map(PropertyRow::Color));
                    }
                }
            }
            visible
        };
        if self.property_rows != property_rows {
            self.property_rows = property_rows;
            self.reset_color_list_layout();
        }
    }

    fn reset_color_list_layout(&self) {
        // The native list measures lazily, so seed plausible row heights for
        // its scrollbar to reach schema rows that have not been rendered yet.
        self.color_list.reset_with_uniform_height(
            self.property_rows.len(),
            render::COLOR_ROW_HEIGHT.to_pixels(self.rem_size),
        );
    }

    fn set_color_origin(
        &mut self,
        origin: colors::ColorOrigin,
        selected: bool,
        cx: &mut Context<Self>,
    ) {
        if self.close_active_picker(cx).is_some() {
            self.color_list.remeasure();
        }
        self.color_origins.set(origin, selected);
        self.refresh_color_list(cx);
        cx.notify();
    }

    fn reset_property_scroll(&self) {
        self.color_list.scroll_to(gpui_kit::ListOffset::default());
        self.roles_list.scroll_to(gpui_kit::ListOffset::default());
        self.scroll
            .set_offset(gpui_kit::point(gpui_kit::px(0.), gpui_kit::px(0.)));
    }

    pub(crate) fn reload_variants(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_variants(true, window, cx);
    }
}

fn make_choices<T: Clone + PartialEq + 'static>(
    items: Vec<Choice<T>>,
    selected: &T,
    window: &mut Window,
    cx: &mut App,
) -> Entity<Choices<T>> {
    let ix = items
        .iter()
        .position(|choice| &choice.value == selected)
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

fn group_choices(category: Category) -> Vec<Choice> {
    category
        .groups()
        .iter()
        .map(|group| Choice::new(*group, *group))
        .collect()
}

fn font_choices(
    document: &themes::ThemeDocument,
    role: FontRole,
    cx: &App,
) -> Vec<Choice<Option<SharedString>>> {
    let selected = document.font(role);
    let catalog = cx.global::<FontCatalog>();
    let mut default = document.clone();
    default.set_font(role, None);
    let resolved = catalog.resolve_roles(&default);
    let fallback = match role {
        FontRole::Interface => resolved.interface(),
        FontRole::Reading => resolved.reading(),
        FontRole::Headings => resolved.headings(),
        FontRole::Code => resolved.code(),
    };
    let mut choices = vec![Choice::new(None, format!("Default ({fallback})"))];
    choices.extend(
        catalog
            .families()
            .iter()
            .map(|family| Choice::new(Some(family.clone()), family.clone())),
    );
    if let Some(selected) = selected
        && !catalog.contains(selected)
    {
        choices.push(Choice::new(
            Some(SharedString::from(selected)),
            format!("{selected} (unavailable)"),
        ));
    }
    choices
}

fn resolved_color(appearance: &ResolvedAppearance, token: &str) -> Option<String> {
    appearance.color(token).map(|color| color.to_hex())
}
