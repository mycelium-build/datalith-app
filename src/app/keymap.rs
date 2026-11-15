use std::collections::BTreeMap;
use std::rc::Rc;

use gpui_kit::{
    App, InvalidKeystrokeError, KeyBinding, KeyBindingContextPredicate, Keystroke, Modifiers,
    OperatingSystem,
};

pub const fn host_os() -> OperatingSystem {
    std::cfg_select! {
        target_os = "macos" => OperatingSystem::Mac,
        target_os = "windows" => OperatingSystem::Windows,
        target_os = "freebsd" => OperatingSystem::FreeBsd,
        target_os = "ios" => OperatingSystem::Ios,
        target_os = "android" => OperatingSystem::Android,
        _ => OperatingSystem::Linux,
    }
}

/// Key context of the main view. Shortcuts anchored here rank below the
/// bindings of a focused text field, so the field keeps its keys.
pub const WORKSPACE_CONTEXT: &str = "Workspace";
/// Key contexts of the document views, set by their file handler.
pub const NOTE_CONTEXT: &str = "Note";
pub const TODO_LIST_CONTEXT: &str = "TodoList";
pub const BASE_CONTEXT: &str = "Base";

/// Where a shortcut applies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShortcutZone {
    /// Everywhere in the workspace.
    Global,
    /// In the workspace, except while a text field has focus.
    Interface,
    /// In a note, in edit and view modes.
    Note,
}

impl ShortcutZone {
    /// The key context predicate that enables a shortcut in this zone.
    /// Every zone sits above a text field in the context stack, so the
    /// field's own bindings win on a conflict.
    const fn predicate(self) -> &'static str {
        match self {
            Self::Global => WORKSPACE_CONTEXT,
            Self::Interface => "Workspace && !Input",
            Self::Note => NOTE_CONTEXT,
        }
    }
}

/// Parse a (possibly `secondary-` prefixed) keystroke for the host OS.
pub fn parse_keystroke(source: &str) -> Result<Keystroke, InvalidKeystrokeError> {
    Keystroke::parse_with_os(source, host_os())
}

fn is_secondary(modifiers: Modifiers) -> bool {
    if host_os() == OperatingSystem::Mac {
        modifiers.platform
    } else {
        modifiers.control
    }
}

use super::actions::{
    CloseTab, CopyPath, Delete, Duplicate, FocusSidebar, GoBack, GoForward, NewFile, NewFolder,
    NewTab, OpenInExplorer, OpenLink, OpenSettings, OpenShortcuts, Quit, Rename, SelectLastTab,
    SelectTab1, SelectTab2, SelectTab3, SelectTab4, SelectTab5, SelectTab6, SelectTab7, SelectTab8,
    ToggleEditorMode, ToggleQuickSwitcher, ToggleSearch, ToggleTheme,
};

/// Shortcuts whose default keys take precedence over a focused text field,
/// e.g. Open link over the editor's own Cmd/Ctrl+Enter.
const DEFAULTS_OVER_FIELDS: &[&str] = &["open-link"];

/// Static shortcut metadata, default binding, and action constructors.
#[derive(Clone, Copy)]
struct ShortcutDefinition {
    id: &'static str,
    category: &'static str,
    binding: &'static str, // a string because GPUI represents it like this
    zone: ShortcutZone,
    description: &'static str,
    action_name: &'static str,
    make_binding: fn(&str, &str, &App) -> Option<KeyBinding>,
}

impl ShortcutDefinition {
    /// The context predicate for `keys`. Datalith's own defaults may also
    /// match inside a text field of the zone, at the field's depth, where
    /// they win because they're registered after the component bindings.
    fn predicate(&self, keys: &str) -> String {
        let zone = self.zone.predicate();
        if DEFAULTS_OVER_FIELDS.contains(&self.id) && same_binding(keys, self.binding) {
            format!("{zone} > Input || {zone}")
        } else {
            zone.to_owned()
        }
    }
}

/// A shortcut with its effective binding after applying user overrides.
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
        assign(&mut next, index, binding);
        self.replace(next, cx)
    }

    fn replace(&mut self, next: Vec<Option<String>>, cx: &mut App) -> anyhow::Result<()> {
        self.bindings = next;
        let persistence = super::settings::set_shortcut_overrides(overrides_for(&self.bindings));
        self.install(cx);
        super::menus::install(cx);
        persistence
    }

    /// Rebuilds the keymap with these shortcuts after every other binding,
    /// which keep their order. Unlike adding `Unbind` entries, this also
    /// removes a shortcut that moves to a context of another depth.
    fn install(&self, cx: &mut App) {
        let shortcuts = SHORTCUTS
            .iter()
            .zip(&self.bindings)
            .filter_map(|(definition, binding)| {
                binding.as_deref().and_then(|keys| {
                    (definition.make_binding)(keys, &definition.predicate(keys), cx)
                })
            })
            .collect::<Vec<_>>();
        let others = other_bindings(cx.key_bindings().borrow().bindings());
        cx.clear_key_bindings();
        cx.bind_keys(others.into_iter().chain(shortcuts));
    }
}

macro_rules! shortcuts {
    ($(($id:literal, $category:literal, $keys:literal, $zone:ident, $description:literal, $action:ident)),* $(,)?) => {
        const SHORTCUTS: &[ShortcutDefinition] = &[
            $(ShortcutDefinition {
                id: $id,
                category: $category,
                binding: $keys,
                zone: ShortcutZone::$zone,
                description: $description,
                action_name: concat!("datalith::", stringify!($action)),
                make_binding: |keys, predicate, cx| {
                    let predicate = KeyBindingContextPredicate::parse(predicate).ok()?;
                    KeyBinding::load(keys, Box::new($action), Some(Rc::new(predicate)), false, None, cx).ok()
                },
            }),*
        ];
    };
}

shortcuts!(
    // `secondary` resolves to Command on macOS and Ctrl on Linux/Windows.
    // File
    ("quit", "File", "secondary-q", Global, "Quit", Quit),
    (
        "new-note",
        "File",
        "secondary-n",
        Global,
        "New note",
        NewFile
    ),
    (
        "new-folder",
        "File",
        "secondary-shift-n",
        Global,
        "New folder",
        NewFolder
    ),
    ("rename", "File", "f2", Global, "Rename", Rename),
    (
        "delete",
        "File",
        "secondary-backspace",
        Interface,
        "Delete",
        Delete
    ),
    (
        "duplicate",
        "File",
        "secondary-d",
        Global,
        "Duplicate",
        Duplicate
    ),
    (
        "open-in-explorer",
        "File",
        "secondary-shift-e",
        Global,
        "Open in Explorer",
        OpenInExplorer
    ),
    (
        "copy-path",
        "File",
        "secondary-l",
        Global,
        "Copy path",
        CopyPath
    ),
    // Navigation
    (
        "quick-switcher",
        "Navigation",
        "secondary-p",
        Global,
        "Quick switcher",
        ToggleQuickSwitcher
    ),
    (
        "focus-sidebar",
        "Navigation",
        "secondary-0",
        Global,
        "Focus sidebar",
        FocusSidebar
    ),
    (
        "navigate-back",
        "Navigation",
        "secondary-[",
        Global,
        "Navigate back",
        GoBack
    ),
    (
        "navigate-forward",
        "Navigation",
        "secondary-]",
        Global,
        "Navigate forward",
        GoForward
    ),
    (
        "open-link",
        "Navigation",
        "secondary-enter",
        Note,
        "Open link",
        OpenLink
    ),
    // Tabs
    ("new-tab", "Tabs", "secondary-t", Global, "New tab", NewTab),
    (
        "close-tab",
        "Tabs",
        "secondary-w",
        Global,
        "Close tab",
        CloseTab
    ),
    (
        "select-tab-1",
        "Tabs",
        "secondary-1",
        Global,
        "Select tab 1",
        SelectTab1
    ),
    (
        "select-tab-2",
        "Tabs",
        "secondary-2",
        Global,
        "Select tab 2",
        SelectTab2
    ),
    (
        "select-tab-3",
        "Tabs",
        "secondary-3",
        Global,
        "Select tab 3",
        SelectTab3
    ),
    (
        "select-tab-4",
        "Tabs",
        "secondary-4",
        Global,
        "Select tab 4",
        SelectTab4
    ),
    (
        "select-tab-5",
        "Tabs",
        "secondary-5",
        Global,
        "Select tab 5",
        SelectTab5
    ),
    (
        "select-tab-6",
        "Tabs",
        "secondary-6",
        Global,
        "Select tab 6",
        SelectTab6
    ),
    (
        "select-tab-7",
        "Tabs",
        "secondary-7",
        Global,
        "Select tab 7",
        SelectTab7
    ),
    (
        "select-tab-8",
        "Tabs",
        "secondary-8",
        Global,
        "Select tab 8",
        SelectTab8
    ),
    (
        "select-last-tab",
        "Tabs",
        "secondary-9",
        Global,
        "Select last tab",
        SelectLastTab
    ),
    // View
    (
        "search-files",
        "View",
        "secondary-shift-f",
        Global,
        "Search files",
        ToggleSearch
    ),
    (
        "toggle-editor-mode",
        "View",
        "secondary-e",
        Global,
        "Toggle edit / view",
        ToggleEditorMode
    ),
    (
        "toggle-theme",
        "View",
        "secondary-shift-d",
        Global,
        "Toggle theme",
        ToggleTheme
    ),
    (
        "open-settings",
        "View",
        "secondary-,",
        Global,
        "Open settings",
        OpenSettings
    ),
    // Help
    (
        "show-shortcuts",
        "Help",
        "secondary-/",
        Global,
        "Show shortcuts",
        OpenShortcuts
    ),
);

pub fn display_binding(binding: &str) -> String {
    parse_keystroke(binding).map_or_else(
        |_| binding.to_owned(),
        |keystroke| keystroke.display(host_os()).to_string(),
    )
}

/// The display label for an optional binding.
pub fn display_binding_or_unbound(binding: Option<&str>) -> String {
    binding.map_or_else(|| "No shortcut".to_owned(), display_binding)
}

pub fn register(cx: &mut App) {
    ShortcutRegistry::new().install(cx);
}

/// The bindings that don't belong to a Datalith shortcut, in keymap order.
fn other_bindings<'a>(bindings: impl Iterator<Item = &'a KeyBinding>) -> Vec<KeyBinding> {
    bindings
        .filter(|binding| {
            let name = binding.action().name();
            SHORTCUTS
                .iter()
                .all(|definition| definition.action_name != name)
        })
        .cloned()
        .collect()
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
            assign(&mut bindings, index, binding);
        }
    }

    bindings
}

/// Give `index` its binding, unbinding any other shortcut with the same keystroke.
fn assign(bindings: &mut [Option<String>], index: usize, binding: Option<String>) {
    if let Some(binding) = binding.as_deref() {
        for (other_index, other) in bindings.iter_mut().enumerate() {
            if other_index != index
                && other
                    .as_deref()
                    .is_some_and(|other| same_binding(other, binding))
            {
                *other = None;
            }
        }
    }
    if let Some(current) = bindings.get_mut(index) {
        *current = binding;
    }
}

fn same_binding(first: &str, second: &str) -> bool {
    matches!(
        (parse_keystroke(first), parse_keystroke(second)),
        (Ok(first), Ok(second)) if first == second
    )
}

pub fn normalize_binding(binding: &str) -> Option<String> {
    let keystroke = parse_keystroke(binding).ok()?;
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
    if is_secondary(*modifiers) {
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
mod tests {
    use super::{
        SHORTCUTS, ShortcutZone, default_bindings, display_binding, effective_bindings,
        overrides_for, parse_keystroke,
    };
    use crate::app::actions::Quit;
    use gpui_kit::{Action, KeyBinding, KeyContext, Keymap, actions};
    use std::collections::BTreeMap;

    actions!(
        shortcut_tests,
        [
            #[derive(Eq)]
            ComponentAction
        ]
    );

    #[cfg(target_os = "macos")]
    const SECONDARY_PREFIX: &str = "cmd";
    #[cfg(not(target_os = "macos"))]
    const SECONDARY_PREFIX: &str = "ctrl";

    fn secondary_keys(suffix: &str) -> &'static str {
        match (SECONDARY_PREFIX, suffix) {
            ("cmd", "q") => "cmd-q",
            ("ctrl", "q") => "ctrl-q",
            ("cmd", "x") => "cmd-x",
            ("ctrl", "x") => "ctrl-x",
            _ => unreachable!("test keys are only q and x"),
        }
    }

    fn binding<A: Action>(suffix: &str, action: A) -> KeyBinding {
        KeyBinding::new(secondary_keys(suffix), action, None)
    }

    fn quit_binding(suffix: &str) -> KeyBinding {
        binding(suffix, Quit)
    }

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
    fn zones_yield_to_text_field_bindings() {
        let ctrl_backspace = [parse_keystroke("ctrl-backspace").expect("valid key")];
        let ctrl_k = [parse_keystroke("ctrl-k").expect("valid key")];
        let keymap = Keymap::new(vec![
            KeyBinding::new("ctrl-backspace", ComponentAction, Some("Input")),
            KeyBinding::new(
                "ctrl-backspace",
                Quit,
                Some(ShortcutZone::Global.predicate()),
            ),
            KeyBinding::new("ctrl-k", Quit, Some(ShortcutZone::Interface.predicate())),
        ]);
        let workspace = KeyContext::parse("Workspace").expect("valid context");
        let input = KeyContext::parse("Input").expect("valid context");
        let typing = [workspace.clone(), input];
        let outside_fields = [workspace];

        let in_field = keymap.bindings_for_input(&ctrl_backspace, &typing).0;
        assert!(in_field[0].action().partial_eq(&ComponentAction));
        let outside = keymap
            .bindings_for_input(&ctrl_backspace, &outside_fields)
            .0;
        assert!(outside[0].action().partial_eq(&Quit));

        assert!(keymap.bindings_for_input(&ctrl_k, &typing).0.is_empty());
        assert!(
            !keymap
                .bindings_for_input(&ctrl_k, &outside_fields)
                .0
                .is_empty()
        );
    }

    #[test]
    fn default_over_fields_beats_the_field_binding() {
        let open_link = SHORTCUTS
            .iter()
            .find(|definition| definition.id == "open-link")
            .expect("open-link is a shortcut");
        let predicate = open_link.predicate(open_link.binding);
        let keymap = Keymap::new(vec![
            KeyBinding::new("ctrl-enter", ComponentAction, Some("Input")),
            KeyBinding::new("ctrl-enter", Quit, Some(&predicate)),
        ]);
        let stack = ["Workspace", "Note", "Input"]
            .map(|context| KeyContext::parse(context).expect("valid context"));

        let ctrl_enter = [parse_keystroke("ctrl-enter").expect("valid key")];
        let matched = keymap.bindings_for_input(&ctrl_enter, &stack).0;
        assert!(matched[0].action().partial_eq(&Quit));
        assert_ne!(open_link.predicate("secondary-o"), predicate);
    }

    #[test]
    fn rebuilding_replaces_only_datalith_shortcuts() {
        let existing = Keymap::new(vec![quit_binding("q"), binding("q", ComponentAction)]);
        let mut keymap = Keymap::new(super::other_bindings(existing.bindings()));
        keymap.add_bindings([quit_binding("x")]);

        let (old_key, _) = keymap.bindings_for_input(
            &[parse_keystroke("secondary-q").expect("valid key")],
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
            &[parse_keystroke("secondary-x").expect("valid key")],
            &[KeyContext::default()],
        );
        assert!(
            new_key
                .iter()
                .any(|binding| binding.action().partial_eq(&Quit))
        );
    }
}
