use super::{THEMES, Workspace};
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
    /// Commands shown in a native action sheet, acting on `target`.
    Actions {
        title: String,
        target: Option<std::path::PathBuf>,
        commands: Vec<super::super::Command>,
    },
    /// The folders above the current one, from the vault root down.
    Ancestor(Vec<std::path::PathBuf>),
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
    pub(crate) fn choose(&mut self, choice: Choice, w: &Window, cx: &mut Context<Self>) {
        let p = &self.preferences;
        let (title, items, current): (&str, Vec<String>, Option<String>) = match choice {
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
            Choice::Actions {
                ref title,
                ref commands,
                ..
            } => {
                let items: Vec<super::super::Item> = commands.iter().map(|c| c.item()).collect();
                w.dismiss_virtual_keyboard();
                let labels = items.iter().map(|i| i.label.to_owned()).collect();
                let anchor = w.mouse_position();
                match crate::commands::actions(title, &items, anchor) {
                    Ok(()) => self.choice = Some((choice, labels)),
                    Err(e) => {
                        self.message = Some(e.to_string());
                        cx.notify();
                    }
                }
                return;
            }
            Choice::Ancestor(ref folders) => (
                "Go to folder",
                folders.iter().map(|p| self.folder_name(p)).collect(),
                None,
            ),
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
            Choice::Actions {
                target, commands, ..
            } => {
                if let Some(command) = usize::try_from(index).ok().and_then(|i| commands.get(i)) {
                    self.run(*command, target, w, cx);
                }
                return;
            }
            Choice::Ancestor(folders) => {
                if let Some(path) = usize::try_from(index).ok().and_then(|i| folders.get(i)) {
                    self.show_folder(&path.clone(), w, cx);
                }
                return;
            }
        }
        self.preferences.apply_theme(w, cx);
        self.save_preferences(cx);
    }
}
