use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Channel {
    Stable,
    Preview,
    Dev,
}

impl Channel {
    pub fn current() -> Self {
        match include_str!("../CHANNEL").trim() {
            "stable" => Self::Stable,
            "preview" => Self::Preview,
            _ => Self::Dev,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Preview => "preview",
            Self::Dev => "dev",
        }
    }

    pub const fn product_name(self) -> &'static str {
        match self {
            Self::Stable => "Datalith",
            Self::Preview => "Datalith Preview",
            Self::Dev => "Datalith Dev",
        }
    }

    pub const fn identifier(self) -> &'static str {
        match self {
            Self::Stable => "build.mycelium.datalith",
            Self::Preview => "build.mycelium.datalith-preview",
            Self::Dev => "build.mycelium.datalith-dev",
        }
    }

    pub const fn stem(self) -> &'static str {
        match self {
            Self::Stable => "datalith",
            Self::Preview => "datalith-preview",
            Self::Dev => "datalith-dev",
        }
    }

    /// Channel-local vault root: `.datalith[/{preview|dev}]`.
    /// Immutable vaults route to `app_data_dir()/immutable-vault-caches/{vault_id}`.
    pub fn vault_dir(self, root: &Path) -> Result<PathBuf> {
        if crate::vault::source::is_read_only(root) {
            let vault = crate::vault::source::embedded_vault(root)
                .filter(|vault| vault.root() == root)
                .context("Unknown immutable vault cache")?;
            return Ok(self
                .app_data_dir()
                .join("immutable-vault-caches")
                .join(vault.id()));
        }
        let directory = root.join(crate::vault::DATALITH_DIR_NAME);
        Ok(match self {
            Self::Stable => directory,
            Self::Preview | Self::Dev => directory.join(self.name()),
        })
    }

    /// Directory holding per-machine workspace JSON for `root`.
    pub fn workspace_dir(self, root: &Path) -> Result<PathBuf> {
        Ok(self.vault_dir(root)?.join("workspace"))
    }

    /// Per-machine workspace JSON file for `root`.
    pub fn workspace_file(self, root: &Path, machine_id: &str) -> Result<PathBuf> {
        ensure!(
            !machine_id.is_empty()
                && machine_id
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() || byte == b'-'),
            "Invalid machine identity"
        );
        Ok(self.workspace_dir(root)?.join(format!("{machine_id}.json")))
    }

    /// Directory holding machine-independent workspace JSON for immutable vaults.
    pub fn immutable_workspace_dir(self) -> PathBuf {
        self.app_data_dir().join("immutable-vaults-workspaces")
    }

    /// OS application-data root for this channel.
    pub fn app_data_dir(self) -> PathBuf {
        #[cfg(test)]
        {
            let test_name = std::thread::current()
                .name()
                .unwrap_or("test")
                .replace("::", "-");
            std::env::temp_dir().join(format!(
                "datalith-test-cache-{}-{}-{test_name}",
                self.stem(),
                std::process::id()
            ))
        }
        #[cfg(not(test))]
        {
            dirs::data_dir().unwrap_or_default().join(self.stem())
        }
    }

    pub const fn update_endpoint(self) -> Option<&'static str> {
        match self {
            Self::Stable => {
                Some("https://mycelium-build.github.io/datalith-app/updates/stable.json")
            }
            Self::Preview => {
                Some("https://mycelium-build.github.io/datalith-app/updates/preview.json")
            }
            Self::Dev => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_dirs_are_separate_and_stable_keeps_its_path() {
        let root = Path::new("vault");
        assert_eq!(
            Channel::Stable.vault_dir(root).unwrap(),
            root.join(".datalith")
        );
        assert_eq!(
            Channel::Preview.vault_dir(root).unwrap(),
            root.join(".datalith/preview")
        );
        assert_eq!(
            Channel::Dev.vault_dir(root).unwrap(),
            root.join(".datalith/dev")
        );
    }

    #[test]
    fn workspace_files_are_per_channel_and_stable_keeps_its_path() {
        let root = Path::new("vault");
        let machine = "a1b2c3";
        assert_eq!(
            Channel::Stable.workspace_file(root, machine).unwrap(),
            root.join(".datalith/workspace/a1b2c3.json")
        );
        assert_eq!(
            Channel::Preview.workspace_file(root, machine).unwrap(),
            root.join(".datalith/preview/workspace/a1b2c3.json")
        );
        assert_eq!(
            Channel::Dev.workspace_file(root, machine).unwrap(),
            root.join(".datalith/dev/workspace/a1b2c3.json")
        );
        assert!(Channel::Stable.workspace_file(root, "").is_err());
        assert!(Channel::Stable.workspace_file(root, "not valid!").is_err());
    }

    #[test]
    fn channels_have_separate_data_and_installation_identities() {
        let channels = [Channel::Stable, Channel::Preview, Channel::Dev];
        for (ix, channel) in channels.iter().enumerate() {
            for other in channels.iter().skip(ix + 1) {
                assert_ne!(channel.stem(), other.stem());
                assert_ne!(channel.identifier(), other.identifier());
                assert_ne!(channel.product_name(), other.product_name());
                assert_ne!(channel.app_data_dir(), other.app_data_dir());
            }
        }
        assert_eq!(Channel::Stable.stem(), "datalith");
        assert_eq!(Channel::Dev.update_endpoint(), None);
        assert_ne!(
            Channel::Stable.update_endpoint(),
            Channel::Preview.update_endpoint()
        );
    }

    #[test]
    fn immutable_vault_caches_are_channel_local_application_data() {
        let root = crate::vault::source::DOCUMENTATION.root();
        let stable = Channel::Stable.vault_dir(&root).unwrap();
        let preview = Channel::Preview.vault_dir(&root).unwrap();
        assert_ne!(stable, preview);
        assert!(stable.ends_with("immutable-vault-caches/documentation"));
        assert!(preview.ends_with("immutable-vault-caches/documentation"));
        assert!(!stable.starts_with(&root));
        assert!(!preview.starts_with(&root));
        assert!(
            Channel::Stable
                .vault_dir(&Path::new("datalith-embedded:").join("unknown"))
                .is_err()
        );
    }

    #[test]
    fn immutable_workspace_dir_is_channel_local_application_data() {
        let stable = Channel::Stable.immutable_workspace_dir();
        let preview = Channel::Preview.immutable_workspace_dir();
        assert_ne!(stable, preview);
        assert!(stable.ends_with("immutable-vaults-workspaces"));
        assert!(preview.ends_with("immutable-vaults-workspaces"));
    }
}
