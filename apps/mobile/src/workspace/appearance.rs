use super::{Page, THEMES, Workspace};
use datalith_ui::fonts::ResolvedFonts;
use gpui_kit::component::{
    ActiveTheme as _,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::{App, Context, Global, Styled as _, Window, px};

pub struct FontCatalog {
    pub(super) families: Vec<String>,
    pub(super) defaults: ResolvedFonts,
}
impl Global for FontCatalog {}
impl FontCatalog {
    pub fn install(cx: &mut App) {
        let mut families = cx.text_system().all_font_names();
        families.retain(|f| !f.starts_with('.') && !f.trim().is_empty());
        families.sort_by_cached_key(|f| f.to_lowercase());
        families.dedup();
        cx.set_global(Self {
            families,
            defaults: cx.global::<ResolvedFonts>().clone(),
        });
    }
}

#[derive(Clone)]
pub enum Choice {
    Theme,
    Variant,
    Font(&'static str),
    Navigation,
    File(std::path::PathBuf),
}
impl Workspace {
    pub(super) fn appearance_controls(&self, cx: &Context<Self>) -> Vec<Button> {
        let selected = &self.preferences;
        let mut settings = vec![
            (Choice::Theme, "Theme", selected.theme.clone()),
            (
                Choice::Variant,
                "Variant",
                selected
                    .variant
                    .clone()
                    .unwrap_or_else(|| "Automatic".into()),
            ),
        ];
        for role in ["Interface", "Reading", "Headings", "Code"] {
            let family = selected
                .fonts
                .get(cx.theme().theme_name().as_str())
                .and_then(|m| m.get(role))
                .cloned()
                .unwrap_or_else(|| "Theme default".into());
            settings.push((Choice::Font(role), role, family));
        }
        settings
            .into_iter()
            .map(|(choice, title, value)| {
                Button::new(title)
                    .ghost()
                    .h(px(48.))
                    .label(format!("{title} · {value}"))
                    .on_click(cx.listener(move |s, _, w, cx| s.choose(choice.clone(), w, cx)))
            })
            .collect()
    }
    pub(in crate::workspace) fn choose(
        &mut self,
        choice: Choice,
        w: &Window,
        cx: &mut Context<Self>,
    ) {
        let p = &self.preferences;
        let (title, items, current) = match choice {
            Choice::Theme => (
                "Theme",
                THEMES.iter().map(|s| s.name.to_string()).collect(),
                Some(p.theme.clone()),
            ),
            Choice::Variant => (
                "Variant",
                std::iter::once("Automatic".into())
                    .chain(
                        THEMES
                            .iter()
                            .filter(|s| s.name.as_ref() == p.theme)
                            .flat_map(|s| s.themes.iter().map(|c| c.name.to_string())),
                    )
                    .collect(),
                Some(p.variant.clone().unwrap_or_else(|| "Automatic".into())),
            ),
            Choice::Font(role) => (
                role,
                std::iter::once("Theme default".into())
                    .chain(cx.global::<FontCatalog>().families.iter().cloned())
                    .collect(),
                Some(
                    p.fonts
                        .get(cx.theme().theme_name().as_str())
                        .and_then(|m| m.get(role))
                        .cloned()
                        .unwrap_or_else(|| "Theme default".into()),
                ),
            ),
            Choice::File(_) => (
                "File actions",
                ["Rename", "Move", "Duplicate", "Delete"]
                    .map(str::to_owned)
                    .to_vec(),
                None,
            ),
            Choice::Navigation => {
                let mut items = vec!["Library".into(), "Open documents".into(), "Settings".into()];
                if !self.forward.is_empty() {
                    items.push("Forward".into());
                }
                ("Actions", items, None)
            }
        };
        let active = items
            .iter()
            .position(|v| Some(v) == current.as_ref())
            .and_then(|i| i32::try_from(i).ok())
            .unwrap_or(-1);
        w.dismiss_virtual_keyboard();
        match crate::commands::show(title, &items, active) {
            Ok(()) => self.choice = Some((choice, items)),
            Err(e) => {
                self.message = Some(e.to_string());
                cx.notify();
            }
        }
    }
    pub fn accept_choice(&mut self, index: i32, w: &mut Window, cx: &mut Context<Self>) {
        let Some((choice, items)) = self.choice.take() else {
            return;
        };
        let Some(value) = usize::try_from(index).ok().and_then(|ix| items.get(ix)) else {
            return;
        };
        match choice {
            Choice::Theme => {
                self.preferences.theme.clone_from(value);
                self.preferences.variant = None;
            }
            Choice::Variant => self.preferences.variant = (index != 0).then(|| value.clone()),
            Choice::Font(role) => {
                let map = self
                    .preferences
                    .fonts
                    .entry(cx.theme().theme_name().to_string())
                    .or_default();
                if index == 0 {
                    map.remove(role);
                } else {
                    map.insert(role.into(), value.clone());
                }
            }
            Choice::File(path) => {
                self.perform_file_action(path, value, w, cx);
                return;
            }
            Choice::Navigation => {
                match value.as_str() {
                    "Forward" => self.go_forward(w, cx),
                    "Open documents" => self.page = Some(Page::Documents),
                    "Settings" => self.page = Some(Page::Settings),
                    _ => self.show_library(w, cx),
                }
                cx.notify();
                return;
            }
        }
        self.preferences.apply_theme(w, cx);
        self.save_preferences(cx);
    }
}
