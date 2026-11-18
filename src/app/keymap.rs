use std::collections::BTreeMap;
use std::rc::Rc;

use gpui_kit::{
    App, InvalidKeystrokeError, KeyBinding, KeyBindingContextPredicate, KeyContext, Keystroke,
    Modifiers, OperatingSystem,
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
pub const BASE_GRAPH_CONTEXT: &str = "BaseGraph";
pub const BASE_VIEWER_CONTEXT: &str = "BaseViewer";
/// Key context of the file tree in the sidebar.
pub const FILE_TREE_CONTEXT: &str = "FileTree";
/// Key context of the modal panels (settings, licenses, palette).
pub const MODAL_CONTEXT: &str = "Modal";

/// Where a shortcut applies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShortcutZone {
    /// Everywhere in the workspace.
    Global,
    /// In the sidebar's file tree, which has no text field except while
    /// renaming, so a key needs no modifier there.
    Sidebar,
    /// In a note, in edit and view modes.
    Note,
    /// In a todo list.
    TodoList,
    /// In a base, in edit and view modes.
    Base,
}

impl ShortcutZone {
    /// The key context predicate that enables a shortcut in this zone.
    /// Every zone sits above a text field in the context stack, so the
    /// field's own bindings win on a conflict.
    const fn predicate(self) -> &'static str {
        match self {
            Self::Global => WORKSPACE_CONTEXT,
            Self::Sidebar => FILE_TREE_CONTEXT,
            Self::Note => NOTE_CONTEXT,
            Self::TodoList => TODO_LIST_CONTEXT,
            Self::Base => BASE_CONTEXT,
        }
    }

    /// The name stored in the settings.
    const fn id(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Sidebar => "sidebar",
            Self::Note => "note",
            Self::TodoList => "todo-list",
            Self::Base => "base",
        }
    }

    /// The name shown in the Shortcuts tab.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Global => "Everywhere",
            Self::Sidebar => "Sidebar",
            Self::Note => "Notes",
            Self::TodoList => "Todo lists",
            Self::Base => "Bases",
        }
    }

    /// Whether `keys` can act in this zone. Focus is almost always in a text
    /// field of a note or a todo list, so a key needs a modifier there, or
    /// must be a function key.
    pub(crate) fn accepts(self, keys: &str) -> bool {
        if !matches!(self, Self::Note | Self::TodoList) {
            return true;
        }
        let Ok(keystroke) = parse_keystroke(keys) else {
            return false;
        };
        let modifiers = keystroke.modifiers;
        !types_character(keys) && (modifiers.modified() || is_function_key(&keystroke.key))
    }

    fn from_id(id: &str) -> Option<Self> {
        [
            Self::Global,
            Self::Sidebar,
            Self::Note,
            Self::TodoList,
            Self::Base,
        ]
        .into_iter()
        .find(|zone| zone.id() == id)
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

use super::settings::ShortcutOverride;

use super::actions::{AddSubtask, DeleteTask, SearchTasks, ToggleTask};
use super::actions::{Bold, InsertWikiLink, Italic};
use super::actions::{
    CloseTab, CopyPath, Delete, Duplicate, FocusSidebar, GoBack, GoForward, NewBase, NewFile,
    NewFolder, NewTab, NewTodoList, NextTab, OpenInExplorer, OpenLink, OpenSettings, OpenShortcuts,
    PreviousTab, Quit, Rename, SelectLastTab, SelectTab1, SelectTab2, SelectTab3, SelectTab4,
    SelectTab5, SelectTab6, SelectTab7, SelectTab8, ToggleEditorMode, ToggleQuickSwitcher,
    ToggleSearch, ToggleTheme,
};
use super::actions::{
    CollapseTask, EditTaskDate, ExpandTask, LowerPriority, NewTask, RaisePriority,
};
use super::actions::{NextView, PreviousView, ResetZoom, ZoomIn, ZoomOut};
use super::actions::{OpenDocumentation, OpenThemes, OpenVault, OpenVaultSelector};

/// Representative context stacks, to find where another binding takes a
/// shortcut's keys: a text field's own keys, or a more specific zone.
const PLACES: &[(&str, &[&str])] = &[
    ("while typing", &[WORKSPACE_CONTEXT, NOTE_CONTEXT, "Input"]),
    ("in notes", &[WORKSPACE_CONTEXT, NOTE_CONTEXT]),
    (
        "in todo lists",
        &[WORKSPACE_CONTEXT, TODO_LIST_CONTEXT, "Input"],
    ),
    ("in bases", &[WORKSPACE_CONTEXT, BASE_CONTEXT]),
    (
        "in base graphs",
        &[
            WORKSPACE_CONTEXT,
            BASE_CONTEXT,
            BASE_VIEWER_CONTEXT,
            BASE_GRAPH_CONTEXT,
        ],
    ),
    ("in the sidebar", &[WORKSPACE_CONTEXT, FILE_TREE_CONTEXT]),
];

/// Shortcuts whose default keys take precedence over a focused text field,
/// e.g. Open link over the editor's own Cmd/Ctrl+Enter.
const DEFAULTS_OVER_FIELDS: &[&str] = &[
    "open-link",
    "toggle-task",
    "add-subtask",
    "delete-task",
    "search-tasks",
    "new-task",
    "raise-priority",
    "lower-priority",
    "edit-task-date",
    "collapse-task",
    "expand-task",
];

/// Application-wide shortcuts that stay active in a modal. Every other
/// workspace shortcut is suspended there, so it can't act behind the panel.
const ACTIVE_IN_MODALS: &[&str] = &[
    "quit",
    "open-settings",
    "toggle-theme",
    "quick-switcher",
    "search-files",
];

/// Static shortcut metadata, default binding, and action constructors.
#[derive(Clone, Copy)]
struct ShortcutDefinition {
    id: &'static str,
    category: &'static str,
    binding: &'static str, // a string because GPUI represents it like this
    zone: ShortcutZone,
    /// The zones the action can act in, starting with `zone`.
    zones: &'static [ShortcutZone],
    description: &'static str,
    action_name: &'static str,
    make_binding: fn(&str, &str, &App) -> Option<KeyBinding>,
}

impl ShortcutDefinition {
    /// The context predicate for `keys`. Datalith's own defaults may also
    /// match inside a text field of the zone, at the field's depth, where
    /// they win because they're registered after the component bindings.
    /// Keys that type a character never match inside a text field.
    /// Workspace zones are suspended in modals, except for a few app actions.
    fn predicate(&self, binding: &Binding) -> Option<String> {
        let keys = binding.keys.as_deref()?;
        let zone = binding.zone.predicate();
        let zone = if binding.zone == ShortcutZone::Global && !ACTIVE_IN_MODALS.contains(&self.id) {
            format!("{zone} && !{MODAL_CONTEXT}")
        } else {
            zone.to_owned()
        };
        Some(
            if self.is_default(binding) && DEFAULTS_OVER_FIELDS.contains(&self.id) {
                format!("({zone} > Input) || {zone}")
            } else if types_character(keys) {
                format!("{zone} && !Input")
            } else {
                zone
            },
        )
    }

    fn default_binding(&self) -> Binding {
        Binding {
            keys: Some(self.binding.to_owned()),
            zone: self.zone,
        }
    }

    fn is_default(&self, binding: &Binding) -> bool {
        binding.zone == self.zone
            && binding
                .keys
                .as_deref()
                .is_some_and(|keys| same_binding(keys, self.binding))
    }
}

/// The keys of a shortcut, `None` when removed, and the zone they apply in.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Binding {
    keys: Option<String>,
    zone: ShortcutZone,
}

/// A shortcut with its effective binding after applying user overrides.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Shortcut {
    id: &'static str,
    category: &'static str,
    description: &'static str,
    binding: Option<String>,
    zone: ShortcutZone,
    zones: &'static [ShortcutZone],
}

impl Shortcut {
    pub(crate) const fn zone(&self) -> ShortcutZone {
        self.zone
    }

    /// The zones the user can choose from, starting with the default.
    pub(crate) const fn zones(&self) -> &'static [ShortcutZone] {
        self.zones
    }

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
    bindings: Vec<Binding>,
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
                binding: binding.keys.clone(),
                zone: binding.zone,
                zones: definition.zones,
            })
            .collect()
    }

    pub(crate) fn binding_for(&self, id: &str) -> Option<&str> {
        SHORTCUTS
            .iter()
            .position(|definition| definition.id == id)
            .and_then(|index| self.bindings.get(index))
            .and_then(|binding| binding.keys.as_deref())
    }

    pub(crate) fn set(&mut self, id: &str, binding: &str, cx: &mut App) -> anyhow::Result<()> {
        let keys = normalize_binding(binding)
            .ok_or_else(|| anyhow::anyhow!("That key combination is not supported."))?;
        self.update_one(id, |binding| binding.keys = Some(keys), cx)
    }

    /// Where the shortcut `id` would apply but another binding wins its keys.
    pub(crate) fn shadowed_places(&self, id: &str, cx: &App) -> Vec<&'static str> {
        let Some((definition, binding)) = SHORTCUTS
            .iter()
            .zip(&self.bindings)
            .find(|(definition, _)| definition.id == id)
        else {
            return Vec::new();
        };
        let (Some(keys), Some(predicate)) =
            (binding.keys.as_deref(), definition.predicate(binding))
        else {
            return Vec::new();
        };
        let (Ok(keystroke), Ok(predicate)) = (
            parse_keystroke(keys),
            KeyBindingContextPredicate::parse(&predicate),
        ) else {
            return Vec::new();
        };
        let keymap = cx.key_bindings();
        let keymap = keymap.borrow();
        PLACES
            .iter()
            .filter(|(_, stack)| {
                let stack = stack
                    .iter()
                    .filter_map(|context| KeyContext::parse(context).ok())
                    .collect::<Vec<_>>();
                predicate.depth_of(&stack).is_some()
                    && keymap
                        .bindings_for_input(std::slice::from_ref(&keystroke), &stack)
                        .0
                        .first()
                        .is_none_or(|winner| winner.action().name() != definition.action_name)
            })
            .map(|(place, _)| *place)
            .collect()
    }

    pub(crate) fn set_zone(
        &mut self,
        id: &str,
        zone: ShortcutZone,
        cx: &mut App,
    ) -> anyhow::Result<()> {
        self.update_one(id, |binding| binding.zone = zone, cx)
    }

    pub(crate) fn remove(&mut self, id: &str, cx: &mut App) -> anyhow::Result<()> {
        self.update_one(id, |binding| binding.keys = None, cx)
    }

    pub(crate) fn reset(&mut self, id: &str, cx: &mut App) -> anyhow::Result<()> {
        let Some(definition) = SHORTCUTS.iter().find(|definition| definition.id == id) else {
            anyhow::bail!("Unknown shortcut.");
        };
        let default = definition.default_binding();
        self.update_one(id, |binding| *binding = default, cx)
    }

    pub(crate) fn reset_all(&mut self, cx: &mut App) -> anyhow::Result<()> {
        self.replace(default_bindings(), cx)
    }

    fn update_one(
        &mut self,
        id: &str,
        change: impl FnOnce(&mut Binding),
        cx: &mut App,
    ) -> anyhow::Result<()> {
        let Some((index, definition)) = SHORTCUTS
            .iter()
            .enumerate()
            .find(|(_, definition)| definition.id == id)
        else {
            anyhow::bail!("Unknown shortcut.");
        };
        let mut next = self.bindings.clone();
        let Some(mut binding) = next.get(index).cloned() else {
            anyhow::bail!("Unknown shortcut.");
        };
        change(&mut binding);
        if !definition.zones.contains(&binding.zone) {
            anyhow::bail!("This action is unavailable in that context.");
        }
        assign(&mut next, index, binding);
        self.replace(next, cx)
    }

    fn replace(&mut self, next: Vec<Binding>, cx: &mut App) -> anyhow::Result<()> {
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
                let keys = binding.keys.as_deref()?;
                (definition.make_binding)(keys, &definition.predicate(binding)?, cx)
            })
            .collect::<Vec<_>>();
        let others = other_bindings(cx.key_bindings().borrow().bindings());
        cx.clear_key_bindings();
        cx.bind_keys(others.into_iter().chain(shortcuts));
    }
}

macro_rules! shortcuts {
    ($(($id:literal, $category:literal, $keys:expr, [$zone:ident $(, $zones:ident)*], $description:literal, $action:ident)),* $(,)?) => {
        const SHORTCUTS: &[ShortcutDefinition] = &[
            $(ShortcutDefinition {
                id: $id,
                category: $category,
                binding: $keys,
                zone: ShortcutZone::$zone,
                zones: &[ShortcutZone::$zone $(, ShortcutZone::$zones)*],
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
    ("quit", "File", "secondary-q", [Global], "Quit", Quit),
    (
        "new-note",
        "File",
        "secondary-n",
        [Global, Sidebar],
        "New note",
        NewFile
    ),
    (
        "new-todo-list",
        "File",
        "ctrl-shift-t",
        [Global, Sidebar],
        "New todo list",
        NewTodoList
    ),
    (
        "new-base",
        "File",
        "secondary-alt-b",
        [Global, Sidebar],
        "New base",
        NewBase
    ),
    (
        "new-folder",
        "File",
        "secondary-shift-n",
        [Global, Sidebar],
        "New folder",
        NewFolder
    ),
    ("rename", "File", "f2", [Sidebar, Global], "Rename", Rename),
    (
        "delete",
        "File",
        // The file managers' key: Delete, or Cmd+Backspace in Finder.
        if cfg!(target_os = "macos") {
            "secondary-backspace"
        } else {
            "delete"
        },
        [Sidebar, Global],
        "Delete",
        Delete
    ),
    (
        "duplicate",
        "File",
        "secondary-d",
        [Sidebar, Global],
        "Duplicate",
        Duplicate
    ),
    (
        "open-in-explorer",
        "File",
        "secondary-shift-e",
        [Sidebar, Global],
        "Open in Explorer",
        OpenInExplorer
    ),
    (
        "copy-path",
        "File",
        "secondary-l",
        [Sidebar, Global],
        "Copy path",
        CopyPath
    ),
    // Navigation
    (
        "open-vault",
        "Navigation",
        "secondary-o",
        [Global],
        "Open vault",
        OpenVault
    ),
    (
        "open-vault-selector",
        "Navigation",
        "ctrl-alt-v",
        [Global],
        "Open vault selector",
        OpenVaultSelector
    ),
    (
        "quick-switcher",
        "Navigation",
        "secondary-p",
        [Global],
        "Quick switcher",
        ToggleQuickSwitcher
    ),
    (
        "focus-sidebar",
        "Navigation",
        "secondary-0",
        [Global],
        "Focus sidebar",
        FocusSidebar
    ),
    (
        "navigate-back",
        "Navigation",
        "secondary-[",
        [Global, Sidebar],
        "Navigate back",
        GoBack
    ),
    (
        "navigate-forward",
        "Navigation",
        "secondary-]",
        [Global, Sidebar],
        "Navigate forward",
        GoForward
    ),
    (
        "open-link",
        "Navigation",
        "secondary-enter",
        [Note],
        "Open link",
        OpenLink
    ),
    // Tabs
    (
        "new-tab",
        "Tabs",
        "secondary-t",
        [Global, Sidebar],
        "New tab",
        NewTab
    ),
    (
        "close-tab",
        "Tabs",
        "secondary-w",
        [Global, Sidebar],
        "Close tab",
        CloseTab
    ),
    // Ctrl, not Cmd, on every platform, like browsers.
    (
        "next-tab",
        "Tabs",
        "ctrl-tab",
        [Global, Sidebar],
        "Next tab",
        NextTab
    ),
    (
        "previous-tab",
        "Tabs",
        "ctrl-shift-tab",
        [Global, Sidebar],
        "Previous tab",
        PreviousTab
    ),
    (
        "select-tab-1",
        "Tabs",
        "secondary-1",
        [Global, Sidebar],
        "Select tab 1",
        SelectTab1
    ),
    (
        "select-tab-2",
        "Tabs",
        "secondary-2",
        [Global, Sidebar],
        "Select tab 2",
        SelectTab2
    ),
    (
        "select-tab-3",
        "Tabs",
        "secondary-3",
        [Global, Sidebar],
        "Select tab 3",
        SelectTab3
    ),
    (
        "select-tab-4",
        "Tabs",
        "secondary-4",
        [Global, Sidebar],
        "Select tab 4",
        SelectTab4
    ),
    (
        "select-tab-5",
        "Tabs",
        "secondary-5",
        [Global, Sidebar],
        "Select tab 5",
        SelectTab5
    ),
    (
        "select-tab-6",
        "Tabs",
        "secondary-6",
        [Global, Sidebar],
        "Select tab 6",
        SelectTab6
    ),
    (
        "select-tab-7",
        "Tabs",
        "secondary-7",
        [Global, Sidebar],
        "Select tab 7",
        SelectTab7
    ),
    (
        "select-tab-8",
        "Tabs",
        "secondary-8",
        [Global, Sidebar],
        "Select tab 8",
        SelectTab8
    ),
    (
        "select-last-tab",
        "Tabs",
        "secondary-9",
        [Global, Sidebar],
        "Select last tab",
        SelectLastTab
    ),
    // View
    (
        "search-files",
        "View",
        "secondary-shift-f",
        [Global, Sidebar],
        "Search files",
        ToggleSearch
    ),
    (
        "toggle-editor-mode",
        "View",
        "secondary-e",
        [Global],
        "Toggle edit / view",
        ToggleEditorMode
    ),
    (
        "toggle-theme",
        "View",
        "secondary-shift-d",
        [Global],
        "Toggle theme",
        ToggleTheme
    ),
    (
        "open-settings",
        "View",
        "secondary-,",
        [Global],
        "Open settings",
        OpenSettings
    ),
    (
        "open-themes",
        "View",
        "secondary-alt-,",
        [Global],
        "Open themes",
        OpenThemes
    ),
    (
        "show-shortcuts",
        "View",
        "secondary-/",
        [Global],
        "Open shortcuts",
        OpenShortcuts
    ),
    // Help
    (
        "documentation",
        "Help",
        "f1",
        [Global],
        "Documentation",
        OpenDocumentation
    ),
    // Note
    ("bold", "Note", "secondary-b", [Note], "Bold", Bold),
    ("italic", "Note", "secondary-i", [Note], "Italic", Italic),
    (
        "insert-wiki-link",
        "Note",
        "secondary-k",
        [Note],
        "Insert wiki link",
        InsertWikiLink
    ),
    // Todo list
    (
        "toggle-task",
        "Todo list",
        "enter",
        [TodoList],
        "Complete task",
        ToggleTask
    ),
    (
        "add-subtask",
        "Todo list",
        "shift-enter",
        [TodoList],
        "Add subtask",
        AddSubtask
    ),
    (
        "delete-task",
        "Todo list",
        "secondary-shift-backspace",
        [TodoList],
        "Delete task",
        DeleteTask
    ),
    (
        "search-tasks",
        "Todo list",
        "secondary-f",
        [TodoList],
        "Search tasks",
        SearchTasks
    ),
    (
        "new-task",
        "Todo list",
        "secondary-n",
        [TodoList],
        "New task",
        NewTask
    ),
    (
        "raise-priority",
        "Todo list",
        "alt-up",
        [TodoList],
        "Raise priority",
        RaisePriority
    ),
    (
        "lower-priority",
        "Todo list",
        "alt-down",
        [TodoList],
        "Lower priority",
        LowerPriority
    ),
    (
        "edit-task-date",
        "Todo list",
        "secondary-d",
        [TodoList],
        "Edit date",
        EditTaskDate
    ),
    (
        "collapse-task",
        "Todo list",
        "secondary-alt-up",
        [TodoList],
        "Collapse subtasks",
        CollapseTask
    ),
    (
        "expand-task",
        "Todo list",
        "secondary-alt-down",
        [TodoList],
        "Expand subtasks",
        ExpandTask
    ),
    // Base
    (
        "next-view",
        "Base",
        "secondary-alt-right",
        [Base],
        "Next view",
        NextView
    ),
    (
        "previous-view",
        "Base",
        "secondary-alt-left",
        [Base],
        "Previous view",
        PreviousView
    ),
    (
        "zoom-in",
        "Base",
        "secondary-=",
        [Base],
        "Zoom in graph",
        ZoomIn
    ),
    (
        "zoom-out",
        "Base",
        "secondary--",
        [Base],
        "Zoom out graph",
        ZoomOut
    ),
    (
        "reset-zoom",
        "Base",
        "secondary-alt-0",
        [Base],
        "Reset graph zoom",
        ResetZoom
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

fn default_bindings() -> Vec<Binding> {
    SHORTCUTS
        .iter()
        .map(ShortcutDefinition::default_binding)
        .collect()
}

fn overrides_for(bindings: &[Binding]) -> BTreeMap<String, ShortcutOverride> {
    SHORTCUTS
        .iter()
        .zip(bindings)
        .filter(|(definition, binding)| !definition.is_default(binding))
        .map(|(definition, binding)| {
            let shortcut = ShortcutOverride {
                keys: binding.keys.clone(),
                zone: (binding.zone != definition.zone).then(|| binding.zone.id().to_owned()),
            };
            (definition.id.to_owned(), shortcut)
        })
        .collect()
}

/// Applies saved overrides to the defaults. Invalid keys are ignored, and a
/// zone the action can't act in falls back to its default zone.
fn effective_bindings(overrides: &BTreeMap<String, ShortcutOverride>) -> Vec<Binding> {
    let mut bindings = default_bindings();
    for (index, definition) in SHORTCUTS.iter().enumerate() {
        let Some(shortcut) = overrides.get(definition.id) else {
            continue;
        };
        let keys = match shortcut.keys.as_deref() {
            // Built-in keys such as Ctrl+Tab can be saved with a custom zone.
            Some(keys) if same_binding(keys, definition.binding) => {
                Some(definition.binding.to_owned())
            }
            Some(keys) => match normalize_binding(keys) {
                Some(keys) => Some(keys),
                None => continue,
            },
            None => None,
        };
        let zone = shortcut
            .zone
            .as_deref()
            .and_then(ShortcutZone::from_id)
            .filter(|zone| definition.zones.contains(zone))
            .unwrap_or(definition.zone);
        assign(&mut bindings, index, Binding { keys, zone });
    }

    bindings
}

/// Give `index` its binding, removing the keys from any other shortcut in
/// the same zone.
fn assign(bindings: &mut [Binding], index: usize, binding: Binding) {
    if let Some(keys) = binding.keys.as_deref() {
        for (other_index, other) in bindings.iter_mut().enumerate() {
            if other_index != index
                && other.zone == binding.zone
                && other
                    .keys
                    .as_deref()
                    .is_some_and(|other| same_binding(other, keys))
            {
                other.keys = None;
            }
        }
    }
    if let Some(current) = bindings.get_mut(index) {
        *current = binding;
    }
}

/// Whether `keys` would type a character in a text field: a printable key
/// alone or with Shift, or on macOS with Option but neither Cmd nor Ctrl.
fn types_character(keys: &str) -> bool {
    let Ok(keystroke) = parse_keystroke(keys) else {
        return false;
    };
    let modifiers = keystroke.modifiers;
    if modifiers.control || modifiers.platform || modifiers.function {
        return false;
    }
    if modifiers.alt && host_os() != OperatingSystem::Mac {
        return false;
    }
    keystroke.key == "space" || keystroke.key.chars().count() == 1
}

fn is_function_key(key: &str) -> bool {
    key.strip_prefix('f')
        .is_some_and(|number| number.parse::<u8>().is_ok_and(|number| number > 0))
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
        DEFAULTS_OVER_FIELDS, MODAL_CONTEXT, SHORTCUTS, ShortcutZone, WORKSPACE_CONTEXT,
        default_bindings, display_binding, effective_bindings, overrides_for, parse_keystroke,
    };
    use crate::app::actions::{OpenLink, Quit};
    use crate::app::settings::ShortcutOverride;
    use gpui_kit::{Action, KeyBinding, KeyBindingContextPredicate, KeyContext, Keymap, actions};
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

    fn index(id: &str) -> usize {
        SHORTCUTS
            .iter()
            .position(|definition| definition.id == id)
            .expect("known shortcut")
    }

    #[test]
    fn overrides_round_trip_keys_zones_and_removals() {
        let mut bindings = default_bindings();
        bindings[index("new-note")].keys = Some("secondary-x".into());
        bindings[index("new-folder")].keys = None;
        bindings[index("rename")].zone = ShortcutZone::Global;
        for id in ["next-tab", "previous-tab"] {
            bindings[index(id)].zone = ShortcutZone::Sidebar;
        }
        let overrides = overrides_for(&bindings);
        assert_eq!(effective_bindings(&overrides), bindings);
    }

    #[test]
    fn keys_move_only_within_the_same_zone() {
        let override_keys = |zone: Option<&str>| ShortcutOverride {
            keys: Some("secondary-x".to_owned()),
            zone: zone.map(str::to_owned),
        };
        let overrides = BTreeMap::from([
            ("new-note".to_owned(), override_keys(None)),
            ("new-folder".to_owned(), override_keys(None)),
            ("rename".to_owned(), override_keys(Some("sidebar"))),
            ("quit".to_owned(), override_keys(Some("note"))),
        ]);
        let effective = effective_bindings(&overrides);
        assert_eq!(
            effective[index("quit")].zone,
            ShortcutZone::Global,
            "zone not permitted"
        );
        assert_eq!(
            effective[index("new-note")].keys,
            None,
            "same zone, later definition wins"
        );
        assert_eq!(
            effective[index("new-folder")].keys.as_deref(),
            Some("secondary-x")
        );
        assert_eq!(
            effective[index("rename")].keys.as_deref(),
            Some("secondary-x")
        );
    }

    #[test]
    fn only_the_sidebar_uses_primary_zero_by_default() {
        let actions = SHORTCUTS
            .iter()
            .filter(|definition| super::same_binding(definition.binding, "secondary-0"))
            .map(|definition| definition.id)
            .collect::<Vec<_>>();
        assert_eq!(actions, ["focus-sidebar"]);
    }

    #[test]
    fn saved_editor_mode_keys_apply_to_both_notes_and_bases() {
        for zone in ["note", "base"] {
            let overrides = BTreeMap::from([(
                "toggle-editor-mode".into(),
                ShortcutOverride {
                    keys: Some("secondary-alt-e".into()),
                    zone: Some(zone.into()),
                },
            )]);
            let bindings = effective_bindings(&overrides);
            let binding = &bindings[index("toggle-editor-mode")];
            assert_eq!(binding.zone, ShortcutZone::Global);
            assert_eq!(binding.keys.as_deref(), Some("secondary-alt-e"));
        }
    }

    #[test]
    fn modals_suspend_workspace_shortcuts_except_app_actions() {
        let workspace = KeyContext::parse(WORKSPACE_CONTEXT).expect("valid context");
        let modal = KeyContext::parse(MODAL_CONTEXT).expect("valid context");
        let input = KeyContext::parse("Input").expect("valid context");
        let in_modal = [workspace.clone(), modal.clone()];
        let typing_in_modal = [workspace.clone(), modal, input];
        let active = |id: &str, stack: &[KeyContext]| {
            let definition = SHORTCUTS
                .iter()
                .find(|definition| definition.id == id)
                .expect("known shortcut");
            let predicate = definition
                .predicate(&definition.default_binding())
                .expect("bound by default");
            KeyBindingContextPredicate::parse(&predicate)
                .expect("valid predicate")
                .depth_of(stack)
                .is_some()
        };

        assert!(active("new-note", std::slice::from_ref(&workspace)));
        assert!(!active("new-note", &in_modal));
        assert!(!active("delete", &in_modal));
        assert!(active("quit", &typing_in_modal));
        assert!(active("quick-switcher", &in_modal));
    }

    #[test]
    fn every_default_parses_and_fits_its_zone() {
        for definition in SHORTCUTS {
            assert!(
                parse_keystroke(definition.binding).is_ok(),
                "{}",
                definition.id
            );
            // Defaults over fields, like Enter in a todo list, may break R1.
            assert!(
                DEFAULTS_OVER_FIELDS.contains(&definition.id)
                    || definition.zone.accepts(definition.binding),
                "{}",
                definition.id
            );
        }
    }

    #[test]
    fn printable_shortcuts_apply_outside_the_text_field() {
        let definition = &SHORTCUTS[index("new-note")];
        let workspace = KeyContext::parse("Workspace").unwrap();
        let typing = [workspace.clone(), KeyContext::parse("Input").unwrap()];
        let active_while_typing = |keys: &str| {
            let binding = super::Binding {
                keys: Some(keys.into()),
                zone: ShortcutZone::Global,
            };
            let predicate =
                KeyBindingContextPredicate::parse(&definition.predicate(&binding).unwrap())
                    .unwrap();
            assert!(
                predicate
                    .depth_of(std::slice::from_ref(&workspace))
                    .is_some(),
                "{keys}"
            );
            predicate.depth_of(&typing).is_some()
        };
        for keys in ["a", "shift-a", "1", "space", "["] {
            assert!(!active_while_typing(keys), "{keys}");
        }
        assert!(active_while_typing("f2"));
        assert_eq!(active_while_typing("alt-5"), !cfg!(target_os = "macos"));
    }

    #[test]
    fn notes_require_a_modifier_or_a_function_key() {
        for keys in ["secondary-k", "shift-enter", "shift-up", "f5"] {
            assert!(ShortcutZone::Note.accepts(keys), "{keys}");
        }
        for keys in ["up", "delete", "shift-a", "space"] {
            assert!(!ShortcutZone::Note.accepts(keys), "{keys}");
        }
        assert!(ShortcutZone::Sidebar.accepts("a"));
    }

    #[test]
    fn open_link_wins_its_default_key_in_the_editor_but_yields_a_remapped_key() {
        let open_link = &SHORTCUTS[index("open-link")];
        let note = [
            KeyContext::parse("Workspace").unwrap(),
            KeyContext::parse("Note").unwrap(),
        ];
        let typing = [
            note[0].clone(),
            note[1].clone(),
            KeyContext::parse("Input").unwrap(),
        ];
        let primary_enter = if cfg!(target_os = "macos") {
            "cmd-enter"
        } else {
            "ctrl-enter"
        };
        for (keys, wins_in_input) in [(primary_enter, true), (secondary_keys("x"), false)] {
            let binding = super::Binding {
                keys: Some(keys.into()),
                zone: ShortcutZone::Note,
            };
            let predicate = open_link.predicate(&binding).unwrap();
            let keymap = Keymap::new(vec![
                KeyBinding::new(keys, ComponentAction, Some("Input")),
                KeyBinding::new(keys, OpenLink, Some(&predicate)),
            ]);
            let pressed = [parse_keystroke(keys).unwrap()];
            assert!(
                keymap.bindings_for_input(&pressed, &note).0[0]
                    .action()
                    .partial_eq(&OpenLink)
            );
            let expected: &dyn Action = if wins_in_input {
                &OpenLink
            } else {
                &ComponentAction
            };
            assert!(
                keymap.bindings_for_input(&pressed, &typing).0[0]
                    .action()
                    .partial_eq(expected)
            );
        }
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
