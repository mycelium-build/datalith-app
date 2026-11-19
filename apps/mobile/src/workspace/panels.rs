use datalith_ui::touch::TouchSize as _;
use std::path::{Component, PathBuf};
use std::sync::LazyLock;

use super::{Workspace, name};
use datalith_vault::{VaultCatalog, source};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Theme, ThemeMode,
    button::{Button, ButtonVariants},
    h_flex,
    list::ListItem,
    theme::ThemeSet,
    v_flex,
};
use gpui_kit::{
    AnyElement, App, Context, IntoElement, ParentElement, Styled, Window, div, prelude::*, px,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Page {
    Library,
    Documents,
    Drafts,
    Settings,
    About,
    License,
    Notices,
}
impl Page {
    const fn title(self) -> &'static str {
        match self {
            Self::Library => "Vaults",
            Self::Documents => "Open documents",
            Self::Drafts => "Recover local edits",
            Self::Settings => "Settings",
            Self::About => "About Datalith",
            Self::License => "License",
            Self::Notices => "Third-party notices",
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct RecentFolder {
    grant: String,
    id: PathBuf,
    label: String,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Preferences {
    theme: String,
    variant: Option<String>,
    fonts: std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    /// The chosen size, used when the size does not follow the system.
    text_size: u8,
    /// Whether text follows the system size setting. Absent from older
    /// preferences: a size other than the former default 16 was a choice.
    /// Field-level default, so an older file reads `None`, not `Some(true)`.
    #[serde(default)]
    system_text: Option<bool>,
    pub(super) documentation: bool,
    folder: PathBuf,
    pub(super) sessions: std::collections::BTreeMap<String, super::session::Session>,
    bookmark: Option<String>,
    external_active: bool,
    recent: Vec<RecentFolder>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: "Datalith".into(),
            variant: None,
            fonts: std::collections::BTreeMap::new(),
            text_size: 16,
            system_text: Some(true),
            documentation: false,
            folder: PathBuf::new(),
            sessions: std::collections::BTreeMap::new(),
            bookmark: None,
            external_active: false,
            recent: Vec::new(),
        }
    }
}
const MIN_TEXT: u8 = 12;
const MAX_TEXT: u8 = 32;

/// The body text size the platform's accessibility setting asks for.
#[cfg_attr(
    not(any(target_os = "ios", target_os = "android")),
    allow(clippy::missing_const_for_fn)
)]
fn system_text_size() -> Option<f32> {
    #[cfg(target_os = "ios")]
    {
        crate::ios::preferred_text_size()
    }
    #[cfg(target_os = "android")]
    {
        crate::android_folder::preferred_text_size().ok()
    }
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        None
    }
}

static THEMES: LazyLock<Vec<ThemeSet>> = LazyLock::new(|| {
    datalith_ui::themes::THEME_SETS
        .iter()
        .filter_map(|json| serde_json::from_str(json).ok())
        .collect()
});
#[path = "appearance.rs"]
mod settings_choices;
pub use settings_choices::{Choice, FontCatalog};
impl Preferences {
    pub(super) fn load() -> Self {
        super::super::DATA
            .get()
            .and_then(|p| std::fs::read(p.join(".mobile-preferences.json")).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }
    pub(super) fn save(&self) -> anyhow::Result<()> {
        let Some(dir) = super::super::DATA.get() else {
            return Ok(());
        };
        let file = tempfile::NamedTempFile::new_in(dir)?;
        serde_json::to_writer(file.as_file(), self)?;
        file.persist(dir.join(".mobile-preferences.json"))?;
        Ok(())
    }
    /// Whether text follows the system size setting.
    pub(super) fn follows_system_text(&self) -> bool {
        self.system_text.unwrap_or(self.text_size == 16)
    }
    /// The text size in use: the system's body size, or the chosen one.
    pub(super) fn effective_text_size(&self) -> u8 {
        let size = if self.follows_system_text() {
            system_text_size().unwrap_or(16.)
        } else {
            f32::from(self.text_size)
        };
        // Whole points between the smallest and largest supported sizes.
        let size = size.round().clamp(f32::from(MIN_TEXT), f32::from(MAX_TEXT));
        format!("{size:.0}").parse().unwrap_or(16)
    }
    pub(super) fn apply_size(&self, cx: &mut App) {
        Theme::global_mut(cx).font_size = px(f32::from(self.effective_text_size()));
    }
    pub(super) fn apply_theme(&self, w: &mut Window, cx: &mut App) {
        for set in THEMES
            .iter()
            .filter(|t| t.name.as_ref() == "Datalith")
            .chain(THEMES.iter().filter(|t| t.name.as_ref() == self.theme))
        {
            for cfg in set.themes.iter().rev() {
                match cfg.mode {
                    ThemeMode::Light => Theme::global_mut(cx).light_theme = cfg.clone().into(),
                    ThemeMode::Dark => Theme::global_mut(cx).dark_theme = cfg.clone().into(),
                }
            }
        }
        let set = THEMES.iter().find(|set| set.name.as_ref() == self.theme);
        let variant = set.and_then(|s| {
            s.themes
                .iter()
                .find(|v| Some(v.name.as_ref()) == self.variant.as_deref())
        });
        let system = ThemeMode::from(w.appearance());
        let config = variant.or_else(|| {
            set.and_then(|s| {
                s.themes
                    .iter()
                    .find(|v| v.mode == system)
                    .or_else(|| s.themes.first())
            })
        });
        if let Some(config) = config {
            match config.mode {
                ThemeMode::Light => Theme::global_mut(cx).light_theme = config.clone().into(),
                ThemeMode::Dark => Theme::global_mut(cx).dark_theme = config.clone().into(),
            }
            Theme::change(config.mode, Some(w), cx);
        } else {
            Theme::sync_system_appearance(Some(w), cx);
        }
        let defaults = cx.global::<settings_choices::FontCatalog>();
        let overrides = self.fonts.get(cx.theme().theme_name().as_str());
        let selected = |role: &str| {
            overrides
                .and_then(|map| map.get(role))
                .filter(|name| defaults.families.contains(name))
                .map(|s| gpui_kit::SharedString::from(s.clone()))
        };
        let interface = selected("Interface")
            .or_else(|| config.and_then(|v| v.font_family.clone()))
            .unwrap_or_else(|| defaults.defaults.interface.clone());
        let resolved = datalith_ui::fonts::ResolvedFonts {
            reading: selected("Reading").unwrap_or_else(|| interface.clone()),
            headings: selected("Headings").unwrap_or_else(|| defaults.defaults.headings.clone()),
            code: selected("Code")
                .or_else(|| config.and_then(|v| v.mono_font_family.clone()))
                .unwrap_or_else(|| defaults.defaults.code.clone()),
            interface,
        };
        Theme::global_mut(cx).font_family = resolved.interface.clone();
        Theme::global_mut(cx).mono_font_family = resolved.code.clone();
        cx.set_global(resolved);
        Theme::sync_base(cx);
        self.apply_size(cx);
        cx.refresh_windows();
    }
}

impl Workspace {
    pub(in crate::workspace) fn remember_folder(&mut self, cx: &mut Context<Self>) {
        if self.restoring {
            return;
        }
        if let Some(relative) = self
            .folders
            .last()
            .zip(self.root.as_ref())
            .and_then(|(f, r)| f.path.strip_prefix(r).ok())
        {
            self.preferences.folder = relative.to_owned();
            self.save_preferences(cx);
        }
    }
    pub(super) fn save_preferences(&mut self, cx: &mut Context<Self>) {
        if let Err(err) = self.preferences.save() {
            self.message = Some(err.to_string());
        }
        cx.notify();
    }
    pub(super) fn switch_vault(
        &mut self,
        root: PathBuf,
        restore: bool,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let catalog =
            match VaultCatalog::open(&root, datalith_core::file_types::registered_file_types()) {
                Ok(c) => c,
                Err(e) => {
                    self.message = Some(e.to_string());
                    cx.notify();
                    return;
                }
            };
        self.persist_session(cx);
        self.restoring = true;
        let saved = self.preferences.folder.clone();
        self.documents.clear();
        self.active = None;
        self.history.clear();
        self.folders.clear();
        self.deleted = None;
        self.deleted_document = None;
        self.search.folder_only = false;
        self.search.recent.clear();
        self.close_pages();
        self.message = None;
        #[cfg(any(target_os = "ios", target_os = "android"))]
        if self.external.as_ref().is_some_and(|f| f.path != root) {
            self.external = None;
        }
        self.preferences.external_active =
            self.local_root.as_ref() != Some(&root) && !source::is_read_only(&root);
        self.preferences.documentation = source::is_read_only(&root);
        self.catalog = Some(catalog);
        self.root = Some(root.clone());
        let mut dir = root;
        self.select(dir.clone(), w, cx);
        if restore {
            for part in saved.components() {
                let Component::Normal(part) = part else { break };
                if part.to_string_lossy().starts_with('.') {
                    break;
                }
                dir.push(part);
                if dir.is_symlink() || !source::is_dir(&dir) {
                    break;
                }
                self.select(dir.clone(), w, cx);
            }
        }
        self.restore_session(w, cx);
        self.restoring = false;
        self.persist_session(cx);
    }
    pub(super) fn open_page(&mut self, page: Page, w: &Window, cx: &mut Context<Self>) {
        w.dismiss_virtual_keyboard();
        self.file_form = None;
        if let Some(previous) = self.page.replace(page)
            && previous != page
        {
            self.page_trail.retain(|p| *p != page);
            self.page_trail.push(previous);
        }
        self.advance(false);
        cx.notify();
    }
    /// Leaves every page, back to the folder or document beneath them.
    pub(super) fn close_pages(&mut self) {
        self.page = None;
        self.page_trail.clear();
    }
    pub(crate) fn show_library(&mut self, w: &Window, cx: &mut Context<Self>) {
        self.open_page(Page::Library, w, cx);
    }
    pub(crate) fn set_search_scope(&mut self, folder_only: bool, cx: &mut Context<Self>) {
        if self.search.folder_only != folder_only {
            self.search.folder_only = folder_only;
            self.search_changed(cx);
        }
    }
    /// The vault-relative folder holding `path`, unless it is the root.
    pub(crate) fn parent_label(&self, path: &std::path::Path) -> Option<String> {
        let parent = path.strip_prefix(self.root.as_ref()?).ok()?.parent()?;
        (!parent.as_os_str().is_empty()).then(|| parent.display().to_string())
    }
    pub(crate) fn search_changed(&mut self, cx: &mut Context<Self>) {
        self.schedule_session_save(cx);
        self.search.task = None;
        self.search.results = super::search::Results::default();
        cx.notify();
        let query = self.search.query(cx).trim().to_owned();
        let Some(catalog) = self.catalog.clone().filter(|_| !query.is_empty()) else {
            return;
        };
        let scope = self
            .search
            .folder_only
            .then(|| self.folders.last().map(|f| f.path.clone()))
            .flatten();
        let open: Vec<PathBuf> = self.documents.keys().cloned().collect();
        self.search.task = Some(cx.spawn(async move |this, cx| {
            let results = cx
                .background_spawn(async move {
                    let names =
                        super::search::names(catalog.paths(), &open, &query, scope.as_deref());
                    let contents = if super::search::searches_contents(&query) {
                        super::search::contents(catalog.search(&query), &names, scope.as_deref())
                    } else {
                        Vec::new()
                    };
                    super::search::Results { names, contents }
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                s.search.results = results;
                s.search.task = None;
                cx.notify();
            });
        }));
    }

    pub(super) fn render_page(&self, page: Page, cx: &Context<Self>) -> AnyElement {
        let mut body = v_flex()
            .id("mobile-page")
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .px_3()
            .gap_1();
        if !cfg!(target_os = "ios") {
            body = body.child(
                Button::new("page-back")
                    .ghost()
                    .touch_h(48.)
                    .icon(IconName::ChevronLeft)
                    .accessibility_label("Back")
                    .on_click(cx.listener(|s, _, w, cx| s.back(w, cx))),
            );
        }
        body = body.child(div().text_xl().py_2().child(page.title()));
        match page {
            Page::Library => {
                body = body.child(section("Vaults", cx));
                for (id, label, root, read_only) in [
                    ("vault-local", "Datalith", self.local_root.clone(), false),
                    (
                        "vault-documentation",
                        "Documentation",
                        Some(super::super::DOCS.root()),
                        true,
                    ),
                ] {
                    let Some(root) = root else { continue };
                    let current = self.root.as_ref() == Some(&root);
                    body = body.child(
                        vault_row(id, label, read_only.then_some("Read only"), current, cx)
                            .on_click(cx.listener(move |s, _, w, cx| {
                                if s.root.as_ref() == Some(&root) {
                                    s.back(w, cx);
                                } else {
                                    s.switch_vault(root.clone(), false, w, cx);
                                }
                            })),
                    );
                }
                #[cfg(any(target_os = "ios", target_os = "android"))]
                {
                    body = body.children(self.folder_rows(cx));
                }
                let drafts = datalith_vault::draft::pending_paths().map_or(0, |p| p.len());
                if drafts > 0 {
                    body = body.child(section("Recover", cx)).child(
                        setting_row(
                            "drafts",
                            "Recover local edits",
                            Some(format!("{drafts} unsaved")),
                            cx,
                        )
                        .on_click(cx.listener(|s, _, w, cx| s.open_page(Page::Drafts, w, cx))),
                    );
                }
            }
            Page::Drafts => return Self::recover_drafts(body, cx),
            Page::Documents => {
                return self.open_documents(body, cx);
            }
            Page::Settings => {
                body = body.child(self.settings(cx));
            }
            Page::About => {
                body = body
                    .child(
                        setting_row(
                            "version",
                            "Version",
                            Some(env!("CARGO_PKG_VERSION").to_owned()),
                            cx,
                        )
                        .disabled(true),
                    )
                    .child(page_link(Page::License, cx))
                    .child(page_link(Page::Notices, cx))
                    .child(
                        setting_row("source", "Source code", Some("GitHub".into()), cx).on_click(
                            |_, _, cx| {
                                cx.open_url("https://github.com/mycelium-build/datalith-app");
                            },
                        ),
                    );
            }
            Page::License => {
                body = body
                    .child(include_str!("../../../../LICENSE"))
                    .child(include_str!("../../../../LICENSING.md"));
            }
            Page::Notices => {
                return self.notices(body, cx);
            }
        }
        body.into_any_element()
    }

    fn open_documents(
        &self,
        mut body: gpui_kit::Stateful<gpui_kit::Div>,
        cx: &Context<Self>,
    ) -> AnyElement {
        if self.documents.is_empty() {
            body = body.child("No open documents");
        }
        for path in self.documents.keys() {
            let path = path.clone();
            let label = self
                .root
                .as_ref()
                .and_then(|r| path.strip_prefix(r).ok())
                .unwrap_or(&path)
                .display()
                .to_string();
            body = body.child(
                Button::new(format!("open-{label}"))
                    .ghost()
                    .touch_h(48.)
                    .label(label)
                    .on_click(cx.listener(move |s, _, w, cx| s.open(path.clone(), w, cx))),
            );
        }
        body.into_any_element()
    }
    fn recover_drafts(
        mut body: gpui_kit::Stateful<gpui_kit::Div>,
        cx: &Context<Self>,
    ) -> AnyElement {
        match datalith_vault::draft::pending_paths() {
            Ok(paths) if paths.is_empty() => {
                body = body.child("All edits are saved");
            }
            Ok(paths) => {
                body = body.child("Save a copy in your local Datalith vault.");
                for (ix, path) in paths.into_iter().enumerate() {
                    body = body.child(
                        Button::new(("recover-draft", ix))
                            .ghost()
                            .touch_h(48.)
                            .label(name(&path))
                            .on_click(cx.listener(move |s, _, w, cx| {
                                if let Err(e) = s.recover_copy(&path, w, cx) {
                                    s.message = Some(e.to_string());
                                    cx.notify();
                                }
                            })),
                    );
                }
            }
            Err(e) => body = body.child(e.to_string()),
        }
        body.into_any_element()
    }
    fn recover_copy(
        &mut self,
        path: &std::path::Path,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let root = self
            .local_root
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Local vault unavailable"))?;
        let copy = datalith_vault::draft::Edit::open(path, String::new())?.copy_to(&root)?;
        self.switch_vault(root, false, w, cx);
        self.open(copy, w, cx);
        Ok(())
    }

    /// Follows a change of Dynamic Type while the app runs. Android recreates
    /// the Activity on a font scale change, which reads the size again.
    #[cfg(target_os = "ios")]
    pub(crate) fn system_text_size_changed(&self, cx: &mut Context<Self>) {
        if self.preferences.follows_system_text() {
            self.preferences.apply_size(cx);
            cx.refresh_windows();
        }
    }

    fn settings(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let size = self.preferences.effective_text_size();
        let system = self.preferences.follows_system_text();
        let step = |id: &'static str, icon: IconName, label: &'static str, delta: i8| {
            let limit = if delta < 0 {
                size <= MIN_TEXT
            } else {
                size >= MAX_TEXT
            };
            Button::new(id)
                .ghost()
                .touch_size(48.)
                .icon(icon)
                .accessibility_label(label)
                .disabled(limit)
                .on_click(cx.listener(move |s, _, _, cx| {
                    // Adjusting the size leaves the system size for a chosen one.
                    let current = s.preferences.effective_text_size();
                    s.preferences.text_size = current
                        .saturating_add_signed(delta)
                        .clamp(MIN_TEXT, MAX_TEXT);
                    s.preferences.system_text = Some(false);
                    s.preferences.apply_size(cx);
                    s.save_preferences(cx);
                    cx.refresh_windows();
                }))
        };
        let value = if system {
            format!("System · {size}")
        } else {
            size.to_string()
        };
        v_flex()
            .child(section("Appearance", cx))
            .children(self.appearance_controls(cx))
            .child(
                // The value sits under its label so the steppers keep their
                // place at any text size.
                h_flex()
                    .min_h(px(48.))
                    .px_2()
                    .gap_2()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .py_1()
                            .child("Text size")
                            .child(div().text_color(cx.theme().muted_foreground).child(value)),
                    )
                    .child(step(
                        "text-smaller",
                        IconName::Minus,
                        "Decrease text size",
                        -2,
                    ))
                    .child(step("text-larger", IconName::Plus, "Increase text size", 2)),
            )
            .when(!system, |el| {
                el.child(
                    setting_row("text-system", "Use system text size", None, cx).on_click(
                        cx.listener(|s, _, _, cx| {
                            s.preferences.system_text = Some(true);
                            s.preferences.apply_size(cx);
                            s.save_preferences(cx);
                            cx.refresh_windows();
                        }),
                    ),
                )
            })
            .child(section("Datalith", cx))
            .child(page_link(Page::About, cx))
    }
    fn notices(&self, body: gpui_kit::Stateful<gpui_kit::Div>, cx: &Context<Self>) -> AnyElement {
        static SECTIONS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
            include_str!("../../../../THIRD-PARTY-NOTICES.md")
                .split("\n### ")
                .collect()
        });
        let current = self.notice_index.min(SECTIONS.len().saturating_sub(1));
        body.child(
            h_flex().children(
                [
                    ("previous", IconName::ChevronLeft, "Previous notice", false),
                    ("next", IconName::ChevronRight, "Next notice", true),
                ]
                .map(|(id, icon, label, next)| {
                    Button::new(id)
                        .ghost()
                        .touch_size(48.)
                        .icon(icon)
                        .accessibility_label(label)
                        .on_click(cx.listener(move |s, _, _, cx| {
                            s.notice_index = if next {
                                current
                                    .saturating_add(1)
                                    .min(SECTIONS.len().saturating_sub(1))
                            } else {
                                current.saturating_sub(1)
                            };
                            cx.notify();
                        }))
                }),
            ),
        )
        .child(format!(
            "{} / {}",
            current.saturating_add(1),
            SECTIONS.len()
        ))
        .children(SECTIONS.get(current).copied())
        .into_any_element()
    }
}
fn page_link(page: Page, cx: &Context<Workspace>) -> ListItem {
    setting_row(page.title(), page.title(), None, cx)
        .on_click(cx.listener(move |s, _, w, cx| s.open_page(page, w, cx)))
}

/// A section heading in a settings-style page.
fn section(title: &'static str, cx: &Context<Workspace>) -> impl IntoElement + use<> {
    div()
        .px_2()
        .pt_4()
        .pb_1()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(title)
}

/// A row that opens something: its title, an optional current value, then a chevron.
pub(super) fn setting_row(
    id: &'static str,
    title: impl Into<gpui_kit::SharedString>,
    value: Option<String>,
    cx: &Context<Workspace>,
) -> ListItem {
    let title = title.into();
    let label = value
        .as_ref()
        .map_or_else(|| title.clone(), |v| format!("{title}, {v}").into());
    // At accessibility text sizes a label and its value no longer fit side by
    // side: stack them, as iOS and Android settings do.
    let stacked = large_text(cx);
    let value = value.map(|v| {
        div()
            .when(!stacked, |el| el.max_w(gpui_kit::relative(0.5)).truncate())
            .when(stacked, gpui_kit::Styled::whitespace_normal)
            .text_color(cx.theme().muted_foreground)
            .child(v)
    });
    let text = if stacked {
        v_flex()
            .flex_1()
            .min_w_0()
            .py_1()
            .child(div().whitespace_normal().child(title))
            .children(value)
            .into_any_element()
    } else {
        h_flex()
            .flex_1()
            .min_w_0()
            .gap_2()
            .child(div().flex_1().min_w_0().truncate().child(title))
            .children(value)
            .into_any_element()
    };
    ListItem::new(id)
        .min_h(px(48.))
        .px_2()
        .accessibility_label(label)
        .child(
            h_flex()
                .w_full()
                .gap_2()
                .child(text)
                .child(Icon::new(IconName::ChevronRight).size_4()),
        )
}

/// Whether text is at an accessibility size, where rows stack their parts.
fn large_text(cx: &Context<Workspace>) -> bool {
    cx.theme().font_size >= px(24.)
}

/// A vault in the Vaults page: a check marks the open one, a lock a read-only one.
fn vault_row(
    id: impl Into<gpui_kit::ElementId>,
    label: impl Into<gpui_kit::SharedString>,
    detail: Option<&'static str>,
    current: bool,
    cx: &Context<Workspace>,
) -> ListItem {
    let label = label.into();
    let spoken = if current {
        format!("{label}, open").into()
    } else {
        label.clone()
    };
    ListItem::new(id)
        .min_h(px(48.))
        .px_2()
        .accessibility_label(spoken)
        .child(
            h_flex()
                .w_full()
                .gap_3()
                .child(Icon::new(IconName::Folder).size_4())
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .py_1()
                        .child(div().truncate().child(label))
                        .children(detail.map(|d| {
                            h_flex()
                                .gap_1()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(Icon::new(datalith_ui::icons::DatalithIcon::Lock).size_3())
                                .child(d)
                        })),
                )
                .when(current, |row| {
                    row.child(
                        Icon::new(IconName::Check)
                            .size_4()
                            .text_color(cx.theme().primary),
                    )
                }),
        )
}

#[cfg(any(target_os = "ios", target_os = "android"))]
impl Workspace {
    fn folder_rows(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        let mut rows = Vec::new();
        for recent in &self.preferences.recent {
            let grant = recent.grant.clone();
            let id = recent.id.clone();
            let current = self.root.as_ref() == Some(&id);
            let menu_title = recent.label.clone();
            rows.push(
                h_flex()
                    .w_full()
                    .child(
                        vault_row(
                            gpui_kit::SharedString::from(format!("recent:{}", id.display())),
                            recent.label.clone(),
                            None,
                            current,
                            cx,
                        )
                        .flex_1()
                        .min_w_0()
                        .on_click(cx.listener(move |s, _, w, cx| {
                            if let Err(e) = s.open_bookmark(&grant, false, w, cx) {
                                s.message = Some(e.to_string());
                                cx.notify();
                            }
                        })),
                    )
                    .child(
                        Button::new(gpui_kit::SharedString::from(format!(
                            "recent-actions:{}",
                            id.display()
                        )))
                        .ghost()
                        .touch_size(48.)
                        .text_color(cx.theme().muted_foreground)
                        .icon(IconName::Ellipsis)
                        .accessibility_label(format!("Actions for {menu_title}"))
                        .on_click(cx.listener(move |s, _, w, cx| {
                            let choice = super::Choice::Actions {
                                title: menu_title.clone(),
                                target: Some(id.clone()),
                                commands: vec![crate::workspace::Command::RemoveRecent],
                            };
                            s.choose(choice, w, cx);
                        })),
                    )
                    .into_any_element(),
            );
        }
        let choose = ListItem::new("open-external")
            .min_h(px(48.))
            .px_2()
            .accessibility_label("Open folder…")
            .child(
                h_flex()
                    .w_full()
                    .gap_3()
                    .child(Icon::new(IconName::FolderOpen).size_4())
                    .child(div().flex_1().child("Open folder…")),
            );
        #[cfg(target_os = "ios")]
        let choose = choose.on_click(|_, _, _| crate::ios_folder::pick());
        #[cfg(target_os = "android")]
        let choose = choose.on_click(cx.listener(|_, _, w, cx| Self::pick_folder(w, cx)));
        rows.push(choose.into_any_element());
        if !self.preferences.recent.is_empty() {
            rows.push(
                div()
                    .px_2()
                    .pt_1()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .whitespace_normal()
                    .child("Removing a folder from recents keeps its files.")
                    .into_any_element(),
            );
        }
        rows
    }
    pub(super) fn forget_folder(
        &mut self,
        id: &std::path::Path,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(ix) = self.preferences.recent.iter().position(|r| r.id == id) else {
            return;
        };
        if self.root.as_deref() == Some(id)
            && let Some(root) = self.local_root.clone()
        {
            self.switch_vault(root, false, w, cx);
            self.page = Some(Page::Library);
        }
        let recent = self.preferences.recent.remove(ix);
        if self.preferences.bookmark.as_ref() == Some(&recent.grant) {
            self.preferences.bookmark = None;
        }
        #[cfg(target_os = "android")]
        if let Err(e) = crate::android_folder::release(&recent.grant) {
            self.message = Some(e.to_string());
        }
        self.save_preferences(cx);
    }
    #[cfg(target_os = "android")]
    fn pick_folder(w: &Window, cx: &Context<Self>) {
        let result = crate::android_folder::pick();
        cx.spawn_in(w, async move |this, cx| {
            let picked = match result {
                Ok(receiver) => receiver
                    .await
                    .unwrap_or_else(|_| Err(anyhow::anyhow!("Folder picker closed"))),
                Err(e) => Err(e),
            };
            let _ = this.update_in(cx, |s, w, cx| {
                if let Err(e) = picked.and_then(|grant| {
                    grant.map_or(Ok(()), |grant| s.open_bookmark(&grant, false, w, cx))
                }) {
                    s.message = Some(e.to_string());
                    cx.notify();
                }
            });
        })
        .detach();
    }
    pub(super) fn restore_external(
        &mut self,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<bool> {
        if self.preferences.external_active
            && let Some(bookmark) = self.preferences.bookmark.clone()
        {
            self.open_bookmark(&bookmark, true, w, cx)?;
            return Ok(true);
        }
        Ok(false)
    }
    #[cfg(target_os = "ios")]
    pub(crate) fn receive_folder(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(result) = crate::ios_folder::take()
            && let Err(e) = result.and_then(|bookmark| self.open_bookmark(&bookmark, false, w, cx))
        {
            self.message = Some(e.to_string());
            cx.notify();
        }
    }
    fn open_bookmark(
        &mut self,
        bookmark: &str,
        restore: bool,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let folder = crate::external_folder::Folder::resolve(bookmark)?;
        self.switch_vault(folder.path.clone(), restore, w, cx);
        anyhow::ensure!(
            self.root.as_ref() == Some(&folder.path),
            "Could not open external vault"
        );
        self.preferences
            .recent
            .retain(|r| r.id != folder.path && r.grant != bookmark);
        self.preferences.recent.insert(
            0,
            RecentFolder {
                grant: folder.bookmark.clone(),
                id: folder.path.clone(),
                label: folder.label.clone(),
            },
        );
        self.preferences.bookmark = Some(folder.bookmark.clone());
        self.external = Some(folder);
        self.save_preferences(cx);
        Ok(())
    }
}

impl Workspace {
    pub fn open_link(&mut self, url: &str, w: &mut Window, cx: &mut Context<Self>) {
        use datalith_core::deeplink::{DeepLink, parse, resolve_in_vault};
        let Some(DeepLink::OpenNote { vault, path }) = parse(url) else {
            return;
        };
        let local = self.local_root.as_ref().filter(|root| name(root) == vault);
        let docs = super::super::DOCS;
        let mut candidates = Vec::new();
        if let Some(root) = local {
            candidates.push((root.clone(), None::<String>));
        }
        if vault == docs.name() || vault == docs.id() {
            candidates.push((docs.root(), None));
        }
        for recent in &self.preferences.recent {
            if recent.label == vault || recent.id.to_str() == Some(&vault) {
                candidates.push((recent.id.clone(), Some(recent.grant.clone())));
            }
        }
        let result = (|| -> anyhow::Result<()> {
            let [(root, grant)] = candidates.as_slice() else {
                anyhow::bail!(
                    "Cannot uniquely identify the vault ‘{vault}’. Open it from Library."
                );
            };
            let target = resolve_in_vault(root, &path)
                .ok_or_else(|| anyhow::anyhow!("Invalid note path"))?;
            if self.root.as_ref() != Some(root) {
                #[cfg(any(target_os = "ios", target_os = "android"))]
                if let Some(grant) = grant {
                    self.open_bookmark(grant, false, w, cx)?;
                } else {
                    self.switch_vault(root.clone(), false, w, cx);
                }
                #[cfg(not(any(target_os = "ios", target_os = "android")))]
                {
                    let _ = grant;
                    self.switch_vault(root.clone(), false, w, cx);
                }
            }
            anyhow::ensure!(
                self.root.as_ref() == Some(root) && self.in_vault(&target),
                "Cannot open ‘{path}’"
            );
            self.file_form = None;
            self.open(target, w, cx);
            Ok(())
        })();
        if let Err(e) = result {
            self.message = Some(e.to_string());
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_preferences_keep_their_theme_and_gain_session_storage() -> anyhow::Result<()> {
        let prefs: Preferences =
            serde_json::from_str(r#"{"theme":"Catppuccin","text_size":20,"folder":"Notes"}"#)?;
        anyhow::ensure!(prefs.theme == "Catppuccin" && prefs.text_size == 20);
        anyhow::ensure!(!prefs.follows_system_text(), "a size of 20 was chosen");
        anyhow::ensure!(prefs.folder == PathBuf::from("Notes"));
        anyhow::ensure!(
            prefs.sessions.is_empty() && prefs.fonts.is_empty() && prefs.variant.is_none()
        );
        Ok(())
    }

    #[test]
    fn text_follows_the_system_unless_a_size_was_chosen() -> anyhow::Result<()> {
        let default: Preferences = serde_json::from_str(r#"{"text_size":16}"#)?;
        anyhow::ensure!(default.follows_system_text());
        anyhow::ensure!(Preferences::default().follows_system_text());
        let chosen: Preferences = serde_json::from_str(r#"{"text_size":16,"system_text":false}"#)?;
        anyhow::ensure!(!chosen.follows_system_text());
        anyhow::ensure!(chosen.effective_text_size() == 16);
        let huge: Preferences = serde_json::from_str(r#"{"text_size":90,"system_text":false}"#)?;
        anyhow::ensure!(huge.effective_text_size() == MAX_TEXT);
        Ok(())
    }

    #[test]
    fn every_bundled_variant_can_be_selected_and_owns_its_font_preferences() {
        assert_eq!(THEMES.len(), datalith_ui::themes::THEME_SETS.len());
        let mut names = std::collections::BTreeSet::new();
        for set in THEMES.iter() {
            assert!(!set.themes.is_empty());
            for config in &set.themes {
                assert!(
                    names.insert(config.name.to_string()),
                    "Duplicate theme variant {}",
                    config.name
                );
            }
        }
    }
}
