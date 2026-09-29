use std::collections::BTreeMap;

use gpui_kit::{App, KeyBinding, Keystroke, Unbind};

use super::actions::{
    CloseTab, CopyPath, Delete, Duplicate, FocusSidebar, GoBack, GoForward, NewFile, NewFolder,
    NewTab, OpenInExplorer, OpenLink, OpenSettings, OpenShortcuts, Quit, Rename, SelectLastTab,
    SelectTab1, SelectTab2, SelectTab3, SelectTab4, SelectTab5, SelectTab6, SelectTab7, SelectTab8,
    ToggleEditorMode, ToggleQuickSwitcher, ToggleSearch, ToggleTheme,
};

#[derive(Clone, Copy)]
struct ShortcutDefinition {
    id: &'static str,
    category: &'static str,
    binding: &'static str,
    description: &'static str,
    make_binding: fn(&str) -> KeyBinding,
    make_unbind: fn(&str) -> KeyBinding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Shortcut {
    id: &'static str,
    category: &'static str,
    description: &'static str,
    binding: Option<String>,
}

impl Shortcut {
    pub(crate) const fn id(&self) -> &'static str {
        self.id
    }

    pub(crate) const fn category(&self) -> &'static str {
        self.category
    }

    pub(crate) const fn description(&self) -> &'static str {
        self.description
    }

    pub(crate) fn binding(&self) -> Option<&str> {
        self.binding.as_deref()
    }
}

pub struct ShortcutRegistry {
    bindings: Vec<Option<String>>,
}

impl ShortcutRegistry {
    pub(crate) fn new() -> Self {
        let overrides = super::settings::snapshot().shortcut_overrides;
        Self {
            bindings: effective_bindings(&overrides),
        }
    }

    pub(crate) fn shortcuts(&self) -> Vec<Shortcut> {
        SHORTCUTS
            .iter()
            .zip(&self.bindings)
            .map(|(definition, binding)| Shortcut {
                id: definition.id,
                category: definition.category,
                description: definition.description,
                binding: binding.clone(),
            })
            .collect()
    }

    pub(crate) fn binding_for(&self, id: &str) -> Option<&str> {
        SHORTCUTS
            .iter()
            .position(|definition| definition.id == id)
            .and_then(|index| self.bindings.get(index))
            .and_then(Option::as_deref)
    }

    pub(crate) fn set(&mut self, id: &str, binding: &str, cx: &mut App) -> anyhow::Result<()> {
        let binding = normalize_binding(binding)
            .ok_or_else(|| anyhow::anyhow!("That key combination is not supported."))?;
        self.update_one(id, Some(binding), cx)
    }

    pub(crate) fn remove(&mut self, id: &str, cx: &mut App) -> anyhow::Result<()> {
        self.update_one(id, None, cx)
    }

    pub(crate) fn reset(&mut self, id: &str, cx: &mut App) -> anyhow::Result<()> {
        let Some(definition) = SHORTCUTS.iter().find(|definition| definition.id == id) else {
            anyhow::bail!("Unknown shortcut.");
        };
        let binding = Some(definition.binding.to_owned());
        self.update_one(id, binding, cx)
    }

    pub(crate) fn reset_all(&mut self, cx: &mut App) -> anyhow::Result<()> {
        self.replace(default_bindings(), cx)
    }

    fn update_one(
        &mut self,
        id: &str,
        binding: Option<String>,
        cx: &mut App,
    ) -> anyhow::Result<()> {
        let Some(index) = SHORTCUTS.iter().position(|definition| definition.id == id) else {
            anyhow::bail!("Unknown shortcut.");
        };
        let mut next = self.bindings.clone();
        if let Some(binding) = binding.as_deref() {
            for (other_index, current) in next.iter_mut().enumerate() {
                if other_index != index
                    && current
                        .as_deref()
                        .is_some_and(|current| same_binding(current, binding))
                {
                    *current = None;
                }
            }
        }
        if let Some(current) = next.get_mut(index) {
            *current = binding;
        }
        self.replace(next, cx)
    }

    fn replace(&mut self, next: Vec<Option<String>>, cx: &mut App) -> anyhow::Result<()> {
        let previous = std::mem::replace(&mut self.bindings, next);
        let persistence = super::settings::set_shortcut_overrides(overrides_for(&self.bindings));
        cx.bind_keys(binding_operations(&previous, &self.bindings));
        super::menus::install(cx);
        persistence
    }
}

macro_rules! shortcuts {
    ($(($id:literal, $category:literal, $keys:literal, $description:literal, $action:ident)),* $(,)?) => {
        const SHORTCUTS: &[ShortcutDefinition] = &[
            $(ShortcutDefinition {
                id: $id,
                category: $category,
                binding: $keys,
                description: $description,
                make_binding: |keys| KeyBinding::new(keys, $action, None),
                make_unbind: |keys| {
                    KeyBinding::new(keys, Unbind(concat!("datalith::", stringify!($action)).into()), None)
                },
            }),*
        ];
    };
}

shortcuts!(
    // `secondary` resolves to Command on macOS and Ctrl on Linux/Windows.
    // File
    ("quit", "File", "secondary-q", "Quit", Quit),
    ("new-note", "File", "secondary-n", "New note", NewFile),
    (
        "new-folder",
        "File",
        "secondary-shift-n",
        "New folder",
        NewFolder
    ),
    ("rename", "File", "f2", "Rename", Rename),
    ("delete", "File", "secondary-backspace", "Delete", Delete),
    ("duplicate", "File", "secondary-d", "Duplicate", Duplicate),
    (
        "open-in-explorer",
        "File",
        "secondary-shift-e",
        "Open in Explorer",
        OpenInExplorer
    ),
    ("copy-path", "File", "secondary-l", "Copy path", CopyPath),
    // Navigation
    (
        "quick-switcher",
        "Navigation",
        "secondary-p",
        "Quick switcher",
        ToggleQuickSwitcher
    ),
    (
        "focus-sidebar",
        "Navigation",
        "secondary-0",
        "Focus sidebar",
        FocusSidebar
    ),
    (
        "navigate-back",
        "Navigation",
        "secondary-[",
        "Navigate back",
        GoBack
    ),
    (
        "navigate-forward",
        "Navigation",
        "secondary-]",
        "Navigate forward",
        GoForward
    ),
    (
        "open-link",
        "Navigation",
        "secondary-enter",
        "Open link",
        OpenLink
    ),
    // Tabs
    ("new-tab", "Tabs", "secondary-t", "New tab", NewTab),
    ("close-tab", "Tabs", "secondary-w", "Close tab", CloseTab),
    (
        "select-tab-1",
        "Tabs",
        "secondary-1",
        "Select tab 1",
        SelectTab1
    ),
    (
        "select-tab-2",
        "Tabs",
        "secondary-2",
        "Select tab 2",
        SelectTab2
    ),
    (
        "select-tab-3",
        "Tabs",
        "secondary-3",
        "Select tab 3",
        SelectTab3
    ),
    (
        "select-tab-4",
        "Tabs",
        "secondary-4",
        "Select tab 4",
        SelectTab4
    ),
    (
        "select-tab-5",
        "Tabs",
        "secondary-5",
        "Select tab 5",
        SelectTab5
    ),
    (
        "select-tab-6",
        "Tabs",
        "secondary-6",
        "Select tab 6",
        SelectTab6
    ),
    (
        "select-tab-7",
        "Tabs",
        "secondary-7",
        "Select tab 7",
        SelectTab7
    ),
    (
        "select-tab-8",
        "Tabs",
        "secondary-8",
        "Select tab 8",
        SelectTab8
    ),
    (
        "select-last-tab",
        "Tabs",
        "secondary-9",
        "Select last tab",
        SelectLastTab
    ),
    // View
    (
        "search-files",
        "View",
        "secondary-shift-f",
        "Search files",
        ToggleSearch
    ),
    (
        "toggle-editor-mode",
        "View",
        "secondary-e",
        "Toggle edit / view",
        ToggleEditorMode
    ),
    (
        "toggle-theme",
        "View",
        "secondary-shift-d",
        "Toggle theme",
        ToggleTheme
    ),
    (
        "open-settings",
        "View",
        "secondary-,",
        "Open settings",
        OpenSettings
    ),
    // Help
    (
        "show-shortcuts",
        "Help",
        "secondary-/",
        "Show shortcuts",
        OpenShortcuts
    ),
);

pub fn display_binding(binding: &str) -> String {
    Keystroke::parse(binding).map_or_else(|_| binding.to_owned(), |keystroke| keystroke.to_string())
}

pub fn binding_for(id: &str) -> Option<String> {
    ShortcutRegistry::new()
        .binding_for(id)
        .map(ToOwned::to_owned)
}

pub fn register(cx: &mut App) {
    let registry = ShortcutRegistry::new();
    let bindings = registry
        .bindings
        .into_iter()
        .zip(SHORTCUTS)
        .filter_map(|(binding, definition)| binding.map(|keys| (definition.make_binding)(&keys)));
    cx.bind_keys(bindings);
}

/// Replaces just Datalith-owned bindings, leaving component and GPUI bindings intact.
fn binding_operations(previous: &[Option<String>], next: &[Option<String>]) -> Vec<KeyBinding> {
    let mut bindings = Vec::new();
    for (definition, binding) in SHORTCUTS.iter().zip(previous) {
        if let Some(binding) = binding {
            bindings.push((definition.make_unbind)(binding));
        }
    }
    for (definition, binding) in SHORTCUTS.iter().zip(next) {
        if let Some(binding) = binding {
            bindings.push((definition.make_binding)(binding));
        }
    }
    bindings
}

fn default_bindings() -> Vec<Option<String>> {
    SHORTCUTS
        .iter()
        .map(|definition| Some(definition.binding.to_owned()))
        .collect()
}

fn overrides_for(bindings: &[Option<String>]) -> BTreeMap<String, Option<String>> {
    SHORTCUTS
        .iter()
        .zip(bindings)
        .filter_map(|(definition, binding)| match binding {
            Some(binding) if same_binding(binding, definition.binding) => None,
            Some(binding) => Some((definition.id.to_owned(), Some(binding.clone()))),
            None => Some((definition.id.to_owned(), None)),
        })
        .collect()
}

fn effective_bindings(overrides: &BTreeMap<String, Option<String>>) -> Vec<Option<String>> {
    let mut bindings = default_bindings();
    for (index, definition) in SHORTCUTS.iter().enumerate() {
        if let Some(binding) = overrides.get(definition.id) {
            let binding = match binding {
                Some(binding) => match normalize_binding(binding) {
                    Some(binding) => Some(binding),
                    None => continue,
                },
                None => None,
            };
            if let Some(binding) = &binding {
                for (other_index, other_binding) in bindings.iter_mut().enumerate() {
                    if other_index != index
                        && other_binding
                            .as_deref()
                            .is_some_and(|other| same_binding(other, binding))
                    {
                        *other_binding = None;
                    }
                }
            }
            if let Some(current) = bindings.get_mut(index) {
                *current = binding;
            }
        }
    }

    bindings
}

fn same_binding(first: &str, second: &str) -> bool {
    matches!((Keystroke::parse(first), Keystroke::parse(second)), (Ok(first), Ok(second)) if first == second)
}

pub fn normalize_binding(binding: &str) -> Option<String> {
    let keystroke = Keystroke::parse(binding).ok()?;
    if keystroke.key.is_empty() {
        return None;
    }
    if matches!(keystroke.key.as_str(), "escape" | "tab")
        || (keystroke.key == "enter" && !keystroke.modifiers.modified())
    {
        return None;
    }

    Some(serialize_keystroke(&keystroke))
}

fn serialize_keystroke(keystroke: &Keystroke) -> String {
    let modifiers = &keystroke.modifiers;
    let mut parts = Vec::new();
    if modifiers.secondary() {
        parts.push("secondary".to_owned());
    }
    if cfg!(target_os = "macos") {
        if modifiers.control {
            parts.push("ctrl".to_owned());
        }
    } else if modifiers.platform {
        parts.push("super".to_owned());
    }
    if modifiers.alt {
        parts.push("alt".to_owned());
    }
    if modifiers.shift {
        parts.push("shift".to_owned());
    }
    if modifiers.function {
        parts.push("fn".to_owned());
    }
    parts.push(keystroke.key.to_ascii_lowercase());
    parts.join("-")
}

pub fn captured_binding(keystroke: &Keystroke) -> Option<String> {
    if keystroke.key.is_empty() {
        return None;
    }
    normalize_binding(&serialize_keystroke(keystroke))
}

#[cfg(test)]
#[allow(clippy::derive_partial_eq_without_eq)]
mod tests {
    use super::{default_bindings, display_binding, effective_bindings, overrides_for};
    use crate::app::actions::Quit;
    use gpui_kit::{KeyBinding, KeyContext, Keymap, Keystroke, actions};
    use std::collections::BTreeMap;

    actions!(shortcut_tests, [ComponentAction]);

    #[test]
    fn displays_secondary_with_platform_conventions() {
        let expected = if cfg!(target_os = "macos") {
            "⌘⇧F"
        } else {
            "ctrl-shift-F"
        };

        assert_eq!(display_binding("secondary-shift-f"), expected);
    }

    #[test]
    fn captured_primary_modifier_matches_defaults_and_reserved_keys_are_rejected() {
        let primary = if cfg!(target_os = "macos") {
            "cmd-n"
        } else {
            "ctrl-n"
        };
        assert_eq!(
            super::normalize_binding(primary).as_deref(),
            Some("secondary-n")
        );
        assert_eq!(super::normalize_binding("enter"), None);
        assert_eq!(super::normalize_binding("tab"), None);
        assert_eq!(super::normalize_binding("escape"), None);
        assert_eq!(
            super::normalize_binding("secondary-enter").as_deref(),
            Some("secondary-enter")
        );
    }

    #[test]
    fn reassignment_persists_the_released_command_as_unbound() {
        let mut bindings = default_bindings();
        bindings[1] = Some("secondary-x".into());
        bindings[2] = None;
        let overrides = overrides_for(&bindings);
        assert_eq!(overrides.get("new-note"), Some(&Some("secondary-x".into())));
        assert_eq!(overrides.get("new-folder"), Some(&None));

        let effective = effective_bindings(&overrides);
        assert_eq!(effective[1].as_deref(), Some("secondary-x"));
        assert_eq!(effective[2], None);
    }

    #[test]
    fn later_shortcut_definition_wins_conflicting_saved_overrides() {
        let overrides = BTreeMap::from([
            ("new-note".to_owned(), Some("secondary-x".to_owned())),
            ("new-folder".to_owned(), Some("secondary-x".to_owned())),
        ]);
        let effective = effective_bindings(&overrides);
        assert_eq!(effective[1], None);
        assert_eq!(effective[2].as_deref(), Some("secondary-x"));
    }

    #[test]
    fn replacing_a_datalith_binding_preserves_other_actions() {
        let previous = default_bindings();
        let mut next = previous.clone();
        next[0] = Some("secondary-x".into());

        let mut keymap = Keymap::new(vec![
            KeyBinding::new("secondary-q", Quit, None),
            KeyBinding::new("secondary-q", ComponentAction, None),
        ]);
        keymap.add_bindings(super::binding_operations(&previous, &next));

        let (old_key, _) = keymap.bindings_for_input(
            &[Keystroke::parse("secondary-q").expect("valid key")],
            &[KeyContext::default()],
        );
        assert!(
            old_key
                .iter()
                .any(|binding| binding.action().partial_eq(&ComponentAction))
        );
        assert!(
            old_key
                .iter()
                .all(|binding| !binding.action().partial_eq(&Quit))
        );

        let (new_key, _) = keymap.bindings_for_input(
            &[Keystroke::parse("secondary-x").expect("valid key")],
            &[KeyContext::default()],
        );
        assert!(
            new_key
                .iter()
                .any(|binding| binding.action().partial_eq(&Quit))
        );
    }

    #[test]
    fn replacing_bindings_does_not_restore_a_default_after_multiple_edits() {
        let first = default_bindings();
        let mut second = first.clone();
        second[0] = Some("secondary-x".into());
        let mut third = second.clone();
        third[0] = None;

        let mut keymap = Keymap::new(vec![KeyBinding::new("secondary-q", Quit, None)]);
        keymap.add_bindings(super::binding_operations(&first, &second));
        keymap.add_bindings(super::binding_operations(&second, &third));
        let (old_key, _) = keymap.bindings_for_input(
            &[Keystroke::parse("secondary-q").expect("valid key")],
            &[KeyContext::default()],
        );
        assert!(
            old_key
                .iter()
                .all(|binding| !binding.action().partial_eq(&Quit))
        );
    }
}
