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

    pub fn vault_cache_dir(self, root: &std::path::Path) -> std::path::PathBuf {
        let directory = root.join(crate::vault::DATALITH_DIR_NAME);
        match self {
            Self::Stable => directory,
            Self::Preview | Self::Dev => directory.join(self.name()),
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
    fn vault_caches_are_separate_and_stable_keeps_its_path() {
        let root = std::path::Path::new("vault");
        assert_eq!(
            Channel::Stable.vault_cache_dir(root),
            root.join(".datalith")
        );
        assert_eq!(
            Channel::Preview.vault_cache_dir(root),
            root.join(".datalith/preview")
        );
        assert_eq!(
            Channel::Dev.vault_cache_dir(root),
            root.join(".datalith/dev")
        );
    }

    #[test]
    fn channels_have_separate_data_and_installation_identities() {
        let channels = [Channel::Stable, Channel::Preview, Channel::Dev];
        for (ix, channel) in channels.iter().enumerate() {
            for other in channels.iter().skip(ix + 1) {
                assert_ne!(channel.stem(), other.stem());
                assert_ne!(channel.identifier(), other.identifier());
                assert_ne!(channel.product_name(), other.product_name());
            }
        }
        assert_eq!(Channel::Stable.stem(), "datalith");
        assert_eq!(Channel::Dev.update_endpoint(), None);
        assert_ne!(
            Channel::Stable.update_endpoint(),
            Channel::Preview.update_endpoint()
        );
    }
}
