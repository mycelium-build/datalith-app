//! Per-vault workspace state and its local machine identity.

use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Deserializer, Serialize};

use crate::document::handler::ViewMode;

const CURRENT_WORKSPACE_SCHEMA_VERSION: u32 = 1;

pub use super::machine::machine_id;

/// Stable identity for one tab, independent of its document and its current position.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct TabId(String);

impl TabId {
    /// Create a stable identity for a newly created tab.
    pub(crate) fn new() -> Self {
        Self(format!("{:032x}", rand::random::<u128>()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for TabId {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value.len() != 32 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(serde::de::Error::custom("invalid tab identity"));
        }
        Ok(Self(value))
    }
}

/// One saved tab. `path: None` is an intentionally empty tab.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceTab {
    pub(crate) id: TabId,
    pub(crate) path: Option<PathBuf>,
    pub(crate) mode: ViewMode,
}

impl WorkspaceTab {
    pub(crate) const fn new(id: TabId, path: Option<PathBuf>, mode: ViewMode) -> Self {
        Self { id, path, mode }
    }

    pub(crate) const fn id(&self) -> &TabId {
        &self.id
    }
}

/// A snapshot belonging to one vault and one machine.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Workspace {
    pub(crate) tabs: Vec<WorkspaceTab>,
    pub(crate) active_tab_id: Option<TabId>,
    pub(crate) expanded_folders: Vec<PathBuf>,
    pub(crate) sidebar_selection: Option<PathBuf>,
}

impl Workspace {
    /// Load this machine's workspace for `vault_root`.
    ///
    /// `Ok(None)` means no workspace has been saved. A saved workspace with no
    /// tabs is `Ok(Some(Workspace::default()))`.
    pub fn load(vault_root: &Path, machine_id: &str) -> Result<Option<Self>> {
        let file = workspace_file_path(vault_root, machine_id)?;
        let json = match fs::read_to_string(&file) {
            Ok(json) => json,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Failed to read workspace: {}", file.display()));
            }
        };
        let stored: StoredWorkspace = serde_json::from_str(&json)
            .with_context(|| format!("Failed to parse workspace: {}", file.display()))?;
        ensure!(
            stored.schema_version == CURRENT_WORKSPACE_SCHEMA_VERSION,
            "Unsupported workspace schema version {} in {}",
            stored.schema_version,
            file.display()
        );

        let tabs = stored
            .tabs
            .into_iter()
            .map(|tab| {
                Ok(WorkspaceTab::new(
                    tab.id,
                    tab.path
                        .as_deref()
                        .map(|path| path_from_vault_relative(path, vault_root, false))
                        .transpose()?,
                    tab.mode,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let expanded_folders = stored
            .expanded_folders
            .iter()
            .map(|path| path_from_vault_relative(path, vault_root, true))
            .collect::<Result<Vec<_>>>()?;
        let sidebar_selection = stored
            .sidebar_selection
            .as_deref()
            .map(|path| path_from_vault_relative(path, vault_root, true))
            .transpose()?;
        let workspace = Self {
            tabs,
            active_tab_id: stored.active_tab_id,
            expanded_folders,
            sidebar_selection,
        };
        workspace.validate_tab_ids()?;
        Ok(Some(workspace))
    }

    /// Save this workspace atomically, preserving the previous file if the write fails.
    pub fn save(&self, vault_root: &Path, machine_id: &str) -> Result<()> {
        self.validate_tab_ids()?;
        let stored = StoredWorkspace {
            schema_version: CURRENT_WORKSPACE_SCHEMA_VERSION,
            tabs: self
                .tabs
                .iter()
                .map(|tab| {
                    Ok(StoredWorkspaceTab {
                        id: tab.id.clone(),
                        path: tab
                            .path
                            .as_deref()
                            .map(|path| path_to_vault_relative(path, vault_root, false))
                            .transpose()?,
                        mode: tab.mode,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
            active_tab_id: self.active_tab_id.clone(),
            expanded_folders: self
                .expanded_folders
                .iter()
                .map(|path| path_to_vault_relative(path, vault_root, true))
                .collect::<Result<Vec<_>>>()?,
            sidebar_selection: self
                .sidebar_selection
                .as_deref()
                .map(|path| path_to_vault_relative(path, vault_root, true))
                .transpose()?,
        };
        let file = workspace_file_path(vault_root, machine_id)?;
        let json = serde_json::to_vec_pretty(&stored).context("Failed to serialize workspace")?;
        write_atomically(&file, &json)
    }

    fn validate_tab_ids(&self) -> Result<()> {
        let mut ids = HashSet::with_capacity(self.tabs.len());
        for tab in &self.tabs {
            ensure!(
                ids.insert(tab.id.clone()),
                "Workspace contains duplicate tab identities"
            );
        }
        if let Some(active_id) = &self.active_tab_id {
            ensure!(
                ids.contains(active_id),
                "Workspace active tab identity does not exist"
            );
        }
        Ok(())
    }
}

#[derive(Deserialize, Serialize)]
struct StoredWorkspace {
    schema_version: u32,
    tabs: Vec<StoredWorkspaceTab>,
    active_tab_id: Option<TabId>,
    expanded_folders: Vec<String>,
    sidebar_selection: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct StoredWorkspaceTab {
    id: TabId,
    /// `None` explicitly records a new, empty tab.
    path: Option<String>,
    mode: ViewMode,
}

fn workspace_file_path(vault_root: &Path, machine_id: &str) -> Result<PathBuf> {
    if crate::vault::source::is_read_only(vault_root) {
        let vault = crate::vault::source::embedded_vault(vault_root)
            .filter(|vault| vault.root() == vault_root)
            .context("Unknown immutable vault workspace")?;
        return Ok(immutable_workspace_dir().join(format!("{}.json", vault.id())));
    }
    ensure!(
        !machine_id.is_empty()
            && machine_id
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() || byte == b'-'),
        "Invalid machine identity"
    );
    Ok(vault_root
        .join(".datalith")
        .join("workspace")
        .join(format!("{machine_id}.json")))
}

#[cfg(test)]
fn immutable_workspace_dir() -> PathBuf {
    let test_name = std::thread::current()
        .name()
        .unwrap_or("test")
        .replace("::", "-");
    std::env::temp_dir().join(format!(
        "datalith-test-immutable-workspaces-{}-{test_name}",
        std::process::id()
    ))
}

#[cfg(not(test))]
fn immutable_workspace_dir() -> PathBuf {
    super::data_dir().join("immutable-vaults-workspaces")
}

fn path_to_vault_relative(
    path: &Path,
    vault_root: &Path,
    allow_vault_root: bool,
) -> Result<String> {
    let relative = path
        .strip_prefix(vault_root)
        .with_context(|| format!("Workspace path is outside its vault: {}", path.display()))?;
    relative_path_to_portable_string(relative, allow_vault_root)
}

fn relative_path_to_portable_string(path: &Path, allow_vault_root: bool) -> Result<String> {
    let parts = path
        .components()
        .map(|component| match component {
            Component::Normal(part) => Ok(part.to_string_lossy().into_owned()),
            _ => bail!("Workspace path must be relative to its vault"),
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        allow_vault_root || !parts.is_empty(),
        "Workspace tab path cannot be the vault root"
    );
    Ok(parts.join("/"))
}

fn path_from_vault_relative(
    path: &str,
    vault_root: &Path,
    allow_vault_root: bool,
) -> Result<PathBuf> {
    if path.is_empty() {
        ensure!(allow_vault_root, "Workspace tab path cannot be empty");
        return Ok(vault_root.to_path_buf());
    }
    let relative = Path::new(path);
    ensure!(
        relative
            .components()
            .all(|component| matches!(component, Component::Normal(_))),
        "Workspace path must be relative to its vault"
    );
    Ok(vault_root.join(relative))
}

fn write_atomically(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path.parent().context("Workspace path has no parent")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("Failed to create workspace directory: {}", parent.display()))?;
    let file_name = path
        .file_name()
        .context("Workspace path has no file name")?
        .to_string_lossy();
    let temporary = parent.join(format!(".{file_name}.{:032x}.tmp", rand::random::<u128>()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .with_context(|| {
            format!(
                "Failed to create temporary workspace file: {}",
                temporary.display()
            )
        })?;
    let write_result = (|| -> Result<()> {
        file.write_all(contents)
            .with_context(|| format!("Failed to write workspace: {}", temporary.display()))?;
        file.sync_all()
            .with_context(|| format!("Failed to flush workspace: {}", temporary.display()))?;
        drop(file);
        fs::rename(&temporary, path).with_context(|| {
            format!(
                "Failed to replace workspace file {} with {}",
                path.display(),
                temporary.display()
            )
        })?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write_result
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn temp_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "datalith-workspace-{label}-{}-{:032x}",
            std::process::id(),
            rand::random::<u128>()
        ));
        fs::create_dir_all(&root).expect("create temporary workspace root");
        root
    }

    #[test]
    fn missing_and_saved_empty_workspaces_are_distinct_and_files_are_vault_relative() {
        let root = temp_root("empty");
        let machine = "a1";
        assert_eq!(Workspace::load(&root, machine).expect("load missing"), None);
        assert_eq!(
            Workspace::load(&root, "a2").expect("other machine has no workspace"),
            None
        );

        let workspace = Workspace::default();
        workspace
            .save(&root, machine)
            .expect("save empty workspace");
        assert_eq!(
            Workspace::load(&root, machine).expect("load saved empty"),
            Some(Workspace::default())
        );

        let id = TabId::new();
        let document = root.join("notes/work.md");
        let empty_tab = TabId::new();
        let workspace = Workspace {
            tabs: vec![
                WorkspaceTab::new(id.clone(), Some(document.clone()), ViewMode::Edit),
                WorkspaceTab::new(empty_tab, None, ViewMode::View),
            ],
            active_tab_id: Some(id),
            expanded_folders: vec![root.join("notes")],
            sidebar_selection: Some(document),
        };
        workspace
            .save(&root, machine)
            .expect("save populated workspace");

        let file = workspace_file_path(&root, machine).expect("workspace file path");
        let json = fs::read_to_string(file).expect("read workspace file");
        assert!(json.contains("notes/work.md"));
        assert!(!json.contains(root.to_string_lossy().as_ref()));
        assert!(json.contains("\"path\": null"));
        assert_eq!(
            Workspace::load(&root, machine).expect("restore populated workspace"),
            Some(workspace)
        );
        assert_eq!(
            Workspace::load(&root, "a2").expect("other machine remains isolated"),
            None
        );

        let other_vault = temp_root("other-vault");
        Workspace::default()
            .save(&other_vault, machine)
            .expect("save another vault workspace");
        assert_eq!(
            Workspace::load(&other_vault, machine).expect("load another vault"),
            Some(Workspace::default())
        );
        assert_ne!(
            workspace_file_path(&root, machine).expect("first vault workspace path"),
            workspace_file_path(&other_vault, machine).expect("second vault workspace path")
        );

        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(other_vault);
    }

    #[test]
    fn failed_save_and_unknown_schema_preserve_the_last_valid_workspace() {
        let root = temp_root("preserve");
        let machine = "b2";
        let good = Workspace {
            tabs: vec![WorkspaceTab::new(
                TabId::new(),
                Some(root.join("note.md")),
                ViewMode::View,
            )],
            ..Workspace::default()
        };
        good.save(&root, machine).expect("save valid workspace");
        let file = workspace_file_path(&root, machine).expect("workspace file path");
        let valid_bytes = fs::read(&file).expect("read valid workspace");

        let invalid = Workspace {
            tabs: vec![WorkspaceTab::new(
                TabId::new(),
                Some(root.parent().unwrap_or(&root).join("outside.md")),
                ViewMode::Edit,
            )],
            ..Workspace::default()
        };
        assert!(invalid.save(&root, machine).is_err());
        assert_eq!(
            fs::read(&file).expect("read preserved workspace"),
            valid_bytes
        );

        fs::write(
            &file,
            r#"{"schema_version":999,"tabs":[],"active_tab_id":null,"expanded_folders":[],"sidebar_selection":null}"#,
        )
        .expect("write unknown schema");
        let unknown_schema = fs::read(&file).expect("read unknown schema bytes");
        assert!(Workspace::load(&root, machine).is_err());
        assert_eq!(
            fs::read(&file).expect("re-read unknown schema"),
            unknown_schema
        );

        fs::write(
            &file,
            r#"{"schema_version":1,"tabs":[{"id":"not-a-tab-id","path":null,"mode":"view"}],"active_tab_id":null,"expanded_folders":[],"sidebar_selection":null}"#,
        )
        .expect("write invalid tab identity");
        let invalid_tab_id = fs::read(&file).expect("read invalid tab identity");
        assert!(Workspace::load(&root, machine).is_err());
        assert_eq!(
            fs::read(&file).expect("re-read invalid tab identity"),
            invalid_tab_id
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn immutable_workspace_is_local_and_independent_of_machine_identity() {
        let root = crate::vault::source::DOCUMENTATION.root();
        let workspace = Workspace {
            tabs: vec![WorkspaceTab::new(
                TabId::new(),
                Some(root.join("Welcome.md")),
                ViewMode::View,
            )],
            ..Workspace::default()
        };
        workspace.save(&root, "machine-one").unwrap();
        assert_eq!(
            Workspace::load(&root, "machine-two").unwrap(),
            Some(workspace)
        );

        let first = workspace_file_path(&root, "machine-one").expect("first workspace path");
        let second = workspace_file_path(&root, "machine-two").expect("second workspace path");
        assert_eq!(first, second);
        assert!(first.starts_with(immutable_workspace_dir()));
        assert!(first.ends_with("documentation.json"));
        assert!(!first.starts_with(&root));
        assert!(!root.exists());
        let _ = fs::remove_dir_all(immutable_workspace_dir());
    }
}
