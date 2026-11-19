//! One command policy for both hosts. Rust decides which commands a screen
//! offers. `UIKit` shows the title and primary action in its navigation bar
//! and Back, New and More in its bottom toolbar; on the iPhone Duo the system
//! moves both into its vertical bar. Android renders them in a GPUI bar. Hosts reply with stable ids, so a
//! translated label can never select the wrong action.
use super::navigation::Surface;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Back,
    Forward,
    ToggleMode,
    EditDefinition,
    NewNote,
    NewFolder,
    Rename,
    Move,
    Duplicate,
    Reveal,
    Delete,
    Vaults,
    Documents,
    Drafts,
    Settings,
    RemoveRecent,
}

impl Command {
    const ALL: [Self; 16] = [
        Self::Back,
        Self::Forward,
        Self::ToggleMode,
        Self::EditDefinition,
        Self::NewNote,
        Self::NewFolder,
        Self::Rename,
        Self::Move,
        Self::Duplicate,
        Self::Reveal,
        Self::Delete,
        Self::Vaults,
        Self::Documents,
        Self::Drafts,
        Self::Settings,
        Self::RemoveRecent,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Back => "back",
            Self::Forward => "forward",
            Self::ToggleMode => "toggle-mode",
            Self::EditDefinition => "edit-definition",
            Self::NewNote => "new-note",
            Self::NewFolder => "new-folder",
            Self::Rename => "rename",
            Self::Move => "move",
            Self::Duplicate => "duplicate",
            Self::Reveal => "reveal",
            Self::Delete => "delete",
            Self::Vaults => "vaults",
            Self::Documents => "documents",
            Self::Drafts => "drafts",
            Self::Settings => "settings",
            Self::RemoveRecent => "remove-recent",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.id() == id)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Back => "Back",
            Self::Forward => "Forward",
            Self::ToggleMode => "Edit",
            Self::EditDefinition => "Edit definition…",
            Self::NewNote => "New note",
            Self::NewFolder => "New folder",
            Self::Rename => "Rename…",
            Self::Move => "Move…",
            Self::Duplicate => "Duplicate",
            Self::Reveal => "Show in folder",
            Self::Delete => "Delete",
            Self::Vaults => "Vaults",
            Self::Documents => "Open documents",
            Self::Drafts => "Recover local edits",
            Self::Settings => "Settings",
            Self::RemoveRecent => "Remove from recents",
        }
    }

    /// SF Symbol drawn by `UIKit`. Android uses the Datalith pixel icons.
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Back => "chevron.backward",
            Self::Forward => "chevron.forward",
            Self::ToggleMode => "pencil",
            Self::EditDefinition => "curlybraces",
            Self::NewNote => "square.and.pencil",
            Self::NewFolder => "folder.badge.plus",
            Self::Rename => "character.cursor.ibeam",
            Self::Move => "folder",
            Self::Duplicate => "plus.square.on.square",
            Self::Reveal => "list.bullet.indent",
            Self::Delete => "trash",
            Self::Vaults => "books.vertical",
            Self::Documents => "square.stack",
            Self::Drafts => "arrow.counterclockwise",
            Self::Settings => "gearshape",
            Self::RemoveRecent => "minus.circle",
        }
    }

    pub const fn destructive(self) -> bool {
        matches!(self, Self::Delete)
    }

    pub const fn item(self) -> Item {
        Item {
            id: self.id(),
            label: self.label(),
            symbol: self.symbol(),
            destructive: self.destructive(),
        }
    }

    /// Actions offered by a file row's menu. Delete stays last.
    pub fn for_file(directory: bool) -> Vec<Self> {
        let mut commands = vec![Self::Rename, Self::Move];
        if !directory {
            commands.push(Self::Duplicate);
        }
        commands.push(Self::Delete);
        commands
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Item {
    pub id: &'static str,
    pub label: &'static str,
    pub symbol: &'static str,
    pub destructive: bool,
}

/// The document shown by the screen, as far as commands are concerned.
#[derive(Clone, Copy, Debug, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent facts about one document"
)]
pub struct DocumentControls {
    pub writable: bool,
    pub can_toggle: bool,
    pub editing: bool,
    /// A Base: its YAML definition is edited from the menu, not a top-level
    /// pencil, so the views stay the primary use.
    pub definition: bool,
}

/// Facts about the visible screen from which the commands are derived.
#[derive(Clone, Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent facts about one screen"
)]
pub struct Screen {
    pub surface: Surface,
    pub title: Option<String>,
    pub can_back: bool,
    pub can_forward: bool,
    pub vault_writable: bool,
    pub document: Option<DocumentControls>,
    /// The search field has the keyboard.
    pub searching: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ScreenControls {
    pub title: Option<String>,
    pub back: bool,
    pub editing: bool,
    /// The keyboard is up for a note or the search: the bottom bar steps
    /// aside, as the keyboard would cover it.
    pub typing: bool,
    pub primary: Option<Item>,
    pub create: Vec<Item>,
    pub menu: Vec<Vec<Item>>,
}

#[cfg_attr(
    all(target_os = "ios", not(test)),
    expect(dead_code, reason = "UIKit reads the model as JSON")
)]
impl ScreenControls {
    pub fn new(screen: &Screen) -> Self {
        let document = screen
            .document
            .filter(|_| screen.surface == Surface::Document);
        let primary = document
            .filter(|d| d.writable && d.can_toggle && (d.editing || !d.definition))
            .map(|d| {
                let mut item = Command::ToggleMode.item();
                if d.editing {
                    item.label = "Done";
                    item.symbol = "checkmark";
                }
                item
            });
        let browsing = matches!(
            screen.surface,
            Surface::Folder | Surface::Document | Surface::Deleted
        );
        let create = if browsing && screen.vault_writable {
            vec![Command::NewNote.item(), Command::NewFolder.item()]
        } else {
            Vec::new()
        };
        let mut menu = Vec::new();
        if screen.surface != Surface::Form {
            if let Some(d) = document.filter(|d| d.writable) {
                let mut section = Vec::new();
                if d.definition && d.can_toggle && !d.editing {
                    section.push(Command::EditDefinition.item());
                }
                section.extend(
                    [
                        Command::Rename,
                        Command::Move,
                        Command::Duplicate,
                        Command::Reveal,
                    ]
                    .map(Command::item),
                );
                menu.push(section);
            } else if document.is_some() {
                menu.push(vec![Command::Reveal.item()]);
            }
            let mut places = Vec::new();
            if screen.can_forward {
                places.push(Command::Forward.item());
            }
            // A document keeps its menu short enough to fit at large text
            // sizes: Vaults and Settings are one Back away, in the folder's menu.
            if document.is_some() {
                places.push(Command::Documents.item());
            } else {
                places.extend([
                    Command::Vaults.item(),
                    Command::Documents.item(),
                    Command::Settings.item(),
                ]);
            }
            menu.push(places);
            if document.is_some_and(|d| d.writable) {
                menu.push(vec![Command::Delete.item()]);
            }
        }
        Self {
            title: screen.title.clone(),
            back: screen.can_back,
            editing: document.is_some_and(|d| d.editing),
            typing: document.is_some_and(|d| d.editing) || screen.searching,
            primary,
            create,
            menu,
        }
    }

    pub fn menu_commands(&self) -> Vec<Command> {
        self.menu
            .iter()
            .flatten()
            .filter_map(|item| Command::from_id(item.id))
            .collect()
    }

    pub fn create_commands(&self) -> Vec<Command> {
        self.create
            .iter()
            .filter_map(|item| Command::from_id(item.id))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(surface: Surface) -> Screen {
        Screen {
            surface,
            title: None,
            can_back: false,
            can_forward: false,
            vault_writable: true,
            document: None,
            searching: false,
        }
    }

    fn ids(items: &[Item]) -> Vec<&'static str> {
        items.iter().map(|i| i.id).collect()
    }

    #[test]
    fn every_command_round_trips_through_its_id() {
        for command in Command::ALL {
            assert_eq!(Command::from_id(command.id()), Some(command));
        }
        assert_eq!(Command::from_id("Delete"), None);
    }

    #[test]
    fn the_bottom_bar_steps_aside_while_typing() {
        assert!(!ScreenControls::new(&screen(Surface::Folder)).typing);
        let mut searching = screen(Surface::Folder);
        searching.searching = true;
        assert!(ScreenControls::new(&searching).typing);
        let mut editing = screen(Surface::Document);
        editing.document = Some(DocumentControls {
            writable: true,
            can_toggle: true,
            editing: true,
            definition: false,
        });
        assert!(ScreenControls::new(&editing).typing);
    }

    #[test]
    fn vault_root_has_neither_back_nor_forward() {
        let controls = ScreenControls::new(&screen(Surface::Folder));
        assert!(!controls.back && controls.primary.is_none());
        assert!(!controls.menu_commands().contains(&Command::Forward));
        assert_eq!(ids(&controls.create), ["new-note", "new-folder"]);
    }

    #[test]
    fn forward_lives_only_in_the_menu() {
        let mut s = screen(Surface::Folder);
        s.can_forward = true;
        let controls = ScreenControls::new(&s);
        assert_eq!(controls.menu_commands().first(), Some(&Command::Forward));
        assert!(controls.primary.is_none());
    }

    #[test]
    fn writable_document_offers_edit_and_ends_with_delete() {
        let mut s = screen(Surface::Document);
        s.can_back = true;
        s.title = Some("Welcome.md".into());
        s.document = Some(DocumentControls {
            writable: true,
            can_toggle: true,
            editing: false,
            definition: false,
        });
        let controls = ScreenControls::new(&s);
        assert!(controls.back);
        assert_eq!(controls.primary.as_ref().map(|i| i.label), Some("Edit"));
        assert_eq!(
            controls.menu.first().map_or_else(Vec::new, |s| ids(s)),
            ["rename", "move", "duplicate", "reveal"]
        );
        let last = controls.menu.last().and_then(|s| s.last());
        assert!(last.is_some_and(|i| i.id == "delete" && i.destructive));
        assert_eq!(controls.menu_commands().last(), Some(&Command::Delete));
    }

    #[test]
    fn editing_document_offers_done() {
        let mut s = screen(Surface::Document);
        s.document = Some(DocumentControls {
            writable: true,
            can_toggle: true,
            editing: true,
            definition: false,
        });
        let primary = ScreenControls::new(&s).primary;
        assert!(primary.is_some_and(|i| i.label == "Done" && i.id == "toggle-mode"));
    }

    #[test]
    fn read_only_document_has_no_edit_or_delete() {
        let mut s = screen(Surface::Document);
        s.vault_writable = false;
        s.document = Some(DocumentControls {
            writable: false,
            can_toggle: true,
            editing: false,
            definition: false,
        });
        let controls = ScreenControls::new(&s);
        assert!(controls.primary.is_none() && controls.create.is_empty());
        let commands = controls.menu_commands();
        assert!(!commands.contains(&Command::Delete) && !commands.contains(&Command::Rename));
    }

    #[test]
    fn writable_base_edits_its_definition_from_the_menu() {
        let mut s = screen(Surface::Document);
        s.document = Some(DocumentControls {
            writable: true,
            can_toggle: true,
            editing: false,
            definition: true,
        });
        let controls = ScreenControls::new(&s);
        assert!(controls.primary.is_none(), "no pencil for a Base");
        assert_eq!(
            controls.menu_commands().first(),
            Some(&Command::EditDefinition)
        );
        s.document = s.document.map(|d| DocumentControls { editing: true, ..d });
        let editing = ScreenControls::new(&s);
        assert!(editing.primary.as_ref().is_some_and(|i| i.label == "Done"));
        assert!(!editing.menu_commands().contains(&Command::EditDefinition));
    }

    #[test]
    fn read_only_base_has_no_definition_editor() {
        let mut s = screen(Surface::Document);
        s.document = Some(DocumentControls {
            writable: false,
            can_toggle: true,
            editing: false,
            definition: true,
        });
        assert!(
            !ScreenControls::new(&s)
                .menu_commands()
                .contains(&Command::EditDefinition)
        );
    }

    #[test]
    fn forms_keep_their_commands_at_the_bottom() {
        let mut s = screen(Surface::Form);
        s.can_back = true;
        let controls = ScreenControls::new(&s);
        assert!(controls.menu.is_empty() && controls.create.is_empty());
        assert!(controls.back);
    }

    #[test]
    fn controls_serialize_for_the_native_host() -> anyhow::Result<()> {
        let mut s = screen(Surface::Document);
        s.title = Some("Welcome.md".into());
        s.document = Some(DocumentControls {
            writable: true,
            can_toggle: true,
            editing: false,
            definition: false,
        });
        let json: serde_json::Value = serde_json::to_value(ScreenControls::new(&s))?;
        let at = |pointer| json.pointer(pointer).cloned();
        anyhow::ensure!(at("/title") == Some("Welcome.md".into()));
        anyhow::ensure!(at("/primary/id") == Some("toggle-mode".into()));
        anyhow::ensure!(at("/menu/2/0/destructive") == Some(true.into()));
        anyhow::ensure!(at("/create/0/symbol") == Some("square.and.pencil".into()));
        Ok(())
    }

    #[test]
    fn file_rows_offer_duplicate_only_for_files() {
        assert_eq!(
            Command::for_file(false),
            [
                Command::Rename,
                Command::Move,
                Command::Duplicate,
                Command::Delete
            ]
        );
        assert!(!Command::for_file(true).contains(&Command::Duplicate));
    }
}
