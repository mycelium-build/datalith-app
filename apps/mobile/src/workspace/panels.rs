use std::path::{Component, PathBuf};
use std::sync::LazyLock;

use super::{Workspace, name};
use datalith_vault::{VaultCatalog, source};
use gpui_kit::component::{
    ActiveTheme as _, IconName, Theme, ThemeMode,
    button::{Button, ButtonVariants},
    h_flex,
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
    pub(super) const fn parent(self) -> Option<Self> {
        match self {
            Self::Notices | Self::License => Some(Self::About),
            Self::About => Some(Self::Settings),
            Self::Settings | Self::Documents | Self::Drafts => Some(Self::Library),
            Self::Library => None,
        }
    }
    const fn title(self) -> &'static str {
        match self {
            Self::Library => "Library",
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
    text_size: u8,
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
            documentation: false,
            folder: PathBuf::new(),
            sessions: std::collections::BTreeMap::new(),
            bookmark: None,
            external_active: false,
            recent: Vec::new(),
        }
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
    pub(super) fn apply_size(&self, cx: &mut App) {
        Theme::global_mut(cx).font_size = px(f32::from(self.text_size.clamp(12, 26)));
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
        self.backward.clear();
        self.forward.clear();
        self.folders.clear();
        self.deleted = None;
        self.deleted_document = None;
        self.whole_vault = false;
        self.page = None;
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
    #[cfg(target_os = "ios")]
    pub fn open_panel(&mut self, settings: bool, w: &Window, cx: &mut Context<Self>) {
        w.dismiss_virtual_keyboard();
        self.page = Some(if settings {
            Page::Settings
        } else {
            Page::Documents
        });
        self.advance(false);
        cx.notify();
    }
    pub(crate) fn show_library(&mut self, w: &Window, cx: &mut Context<Self>) {
        w.dismiss_virtual_keyboard();
        self.page = Some(Page::Library);
        self.advance(false);
        cx.notify();
    }
    pub(crate) fn library_menu(cx: &Context<Self>) -> impl IntoElement + use<> {
        Button::new("library-tools")
            .ghost()
            .w(px(48.))
            .h(px(48.))
            .icon(IconName::Ellipsis)
            .accessibility_label("Library and settings")
            .on_click(cx.listener(|s, _, w, cx| s.choose(Choice::Navigation, w, cx)))
    }
    pub(crate) const fn is_searching(&self) -> bool {
        self.search_task.is_some()
    }
    pub(crate) fn search_changed(&mut self, cx: &mut Context<Self>) {
        self.schedule_session_save(cx);
        self.search_task = None;
        self.search_results.clear();

        cx.notify();
        let Some(folder) = self.folders.last() else {
            return;
        };
        let query = folder.query(cx).trim().to_lowercase();
        if !self.whole_vault || query.is_empty() {
            return;
        }
        let Some(catalog) = self.catalog.clone() else {
            return;
        };

        self.search_task = Some(cx.spawn(async move |this, cx| {
            let hits = cx
                .background_spawn(async move {
                    let mut paths: Vec<_> = catalog
                        .paths()
                        .into_iter()
                        .filter(|p| name(p).to_lowercase().contains(&query))
                        .collect();
                    let mut seen: std::collections::HashSet<_> = paths.iter().cloned().collect();
                    for hit in catalog.search(&query) {
                        if seen.insert(hit.path.clone()) {
                            paths.push(hit.path);
                        }
                    }
                    paths
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                s.search_results = hits;
                s.search_task = None;
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
                    .h(px(48.))
                    .icon(IconName::ChevronLeft)
                    .accessibility_label("Back")
                    .on_click(cx.listener(|s, _, w, cx| s.back(w, cx))),
            );
        }
        body = body.child(div().text_xl().py_2().child(page.title()));
        match page {
            Page::Library => {
                for (label, root) in [
                    ("Datalith", self.local_root.clone()),
                    ("Documentation · read only", Some(super::super::DOCS.root())),
                ] {
                    let Some(root) = root else { continue };
                    body = body.child(Button::new(label).ghost().h(px(48.)).label(label).on_click(
                        cx.listener(move |s, _, w, cx| {
                            if s.root.as_ref() == Some(&root) {
                                s.page = None;
                                cx.notify();
                            } else {
                                s.switch_vault(root.clone(), false, w, cx);
                            }
                        }),
                    ));
                }
                #[cfg(any(target_os = "ios", target_os = "android"))]
                {
                    body = body.children(self.folder_buttons(cx));
                }
                if self.root.as_ref().is_some_and(|r| !source::is_read_only(r)) {
                    body = body
                        .child(Self::create_button(false, cx))
                        .child(Self::create_button(true, cx));
                }
                body = body
                    .child(page_link(Page::Documents, cx))
                    .child(page_link(Page::Drafts, cx))
                    .child(page_link(Page::Settings, cx));
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
                    .child(format!("Datalith {}", env!("CARGO_PKG_VERSION")))
                    .child(page_link(Page::License, cx))
                    .child(page_link(Page::Notices, cx))
                    .child(
                        Button::new("source")
                            .ghost()
                            .h(px(48.))
                            .label("Source code")
                            .on_click(|_, _, cx| {
                                cx.open_url("https://github.com/mycelium-build/datalith-app");
                            }),
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
                    .h(px(48.))
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
                            .h(px(48.))
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

    fn settings(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        v_flex()
            .children(self.appearance_controls(cx))
            .child(
                h_flex()
                    .child(
                        div()
                            .flex_1()
                            .child(format!("Text size · {}", self.preferences.text_size)),
                    )
                    .children([("smaller", "−", -2_i8), ("larger", "+", 2)].map(
                        |(id, label, delta)| {
                            Button::new(id)
                                .ghost()
                                .h(px(48.))
                                .w(px(48.))
                                .label(label)
                                .accessibility_label(id)
                                .on_click(cx.listener(move |s, _, _, cx| {
                                    s.preferences.text_size = s
                                        .preferences
                                        .text_size
                                        .saturating_add_signed(delta)
                                        .clamp(12, 26);
                                    s.preferences.apply_size(cx);
                                    s.save_preferences(cx);
                                    cx.refresh_windows();
                                }))
                        },
                    )),
            )
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
            h_flex().children([("previous", "‹", false), ("next", "›", true)].map(
                |(id, label, next)| {
                    Button::new(id)
                        .ghost()
                        .h(px(48.))
                        .label(label)
                        .accessibility_label(id)
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
                },
            )),
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
fn page_link(page: Page, cx: &Context<Workspace>) -> Button {
    Button::new(page.title())
        .ghost()
        .h(px(48.))
        .label(page.title())
        .on_click(cx.listener(move |s, _, w, cx| {
            w.dismiss_virtual_keyboard();
            s.page = Some(page);
            s.advance(false);
            cx.notify();
        }))
}

#[cfg(any(target_os = "ios", target_os = "android"))]
impl Workspace {
    fn folder_buttons(&self, cx: &Context<Self>) -> Vec<Button> {
        let choose = Button::new("open-external")
            .ghost()
            .h(px(48.))
            .label("Open external folder…");
        #[cfg(target_os = "ios")]
        let choose = choose.on_click(|_, _, _| crate::ios_folder::pick());
        #[cfg(target_os = "android")]
        let choose = choose.on_click(cx.listener(|s, _, w, cx| s.pick_folder(w, cx)));
        let mut buttons = vec![choose];
        for (ix, recent) in self.preferences.recent.iter().enumerate() {
            let grant = recent.grant.clone();
            let id = recent.id.clone();
            buttons.push(
                Button::new(("recent-folder", ix))
                    .ghost()
                    .h(px(48.))
                    .label(recent.label.clone())
                    .on_click(cx.listener(move |s, _, w, cx| {
                        if let Err(e) = s.open_bookmark(&grant, false, w, cx) {
                            s.message = Some(e.to_string());
                            cx.notify();
                        }
                    })),
            );
            let label = format!("Forget {}", recent.label);
            buttons.push(
                Button::new(("forget-folder", ix))
                    .ghost()
                    .icon(IconName::Close)
                    .accessibility_label(label)
                    .on_click(cx.listener(move |s, _, w, cx| {
                        s.forget_folder(&id, w, cx);
                    })),
            );
        }
        buttons
    }
    fn forget_folder(&mut self, id: &std::path::Path, w: &mut Window, cx: &mut Context<Self>) {
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
    fn pick_folder(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let result = crate::android_folder::pick();
        cx.spawn_in(w, async move |this, cx| {
            let picked = match result {
                Ok(receiver) => receiver
                    .await
                    .unwrap_or_else(|_| Err(anyhow::anyhow!("Folder picker closed"))),
                Err(e) => Err(e),
            };
            let _ = this.update_in(cx, |s, w, cx| {
                if let Err(e) = picked.and_then(|grant| match grant {
                    Some(grant) => s.open_bookmark(&grant, false, w, cx),
                    None => Ok(()),
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
        assert_eq!(prefs.theme, "Catppuccin");
        assert_eq!(prefs.text_size, 20);
        assert_eq!(prefs.folder, PathBuf::from("Notes"));
        assert!(prefs.sessions.is_empty());
        assert!(prefs.fonts.is_empty());
        assert!(prefs.variant.is_none());
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
