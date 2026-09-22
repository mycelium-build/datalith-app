//! The workspace restored when the application opens again.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{docs, settings::ApplicationSettings};

/// A saved workspace. `Some(Session::default())` is an intentionally empty session.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct Session {
    pub(crate) vault: Option<PathBuf>,
    pub(crate) tabs: Vec<PathBuf>,
    /// Catalog context by position in this saved tab list, before missing tabs are skipped.
    /// Older sessions have no context and retain the current-Vault fallback.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) tab_vaults: BTreeMap<usize, PathBuf>,
    /// Position in this saved `tabs` snapshot, remapped when unavailable files are skipped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) active_tab_index: Option<usize>,
    /// Legacy sessions selected the first tab matching this path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) active_tab: Option<PathBuf>,
    pub(crate) expanded_folders: Vec<PathBuf>,
    pub(crate) sidebar_selection: Option<PathBuf>,
}

impl Session {
    /// Restore a saved session, or welcome a new user when no workspace exists yet.
    pub(crate) fn initial(
        settings: &ApplicationSettings,
        docs_vault: Option<PathBuf>,
    ) -> (bool, Self) {
        if let Some(session) = &settings.session {
            return (false, session.clone());
        }
        if let Some(vault) = &settings.last_vault {
            return (
                false,
                Self {
                    vault: Some(vault.clone()),
                    ..Self::default()
                },
            );
        }
        let active_tab = docs_vault
            .as_ref()
            .map(|root| root.join(docs::WELCOME_NOTE));
        (
            true,
            Self {
                vault: docs_vault,
                tabs: active_tab.iter().cloned().collect(),
                active_tab_index: active_tab.map(|_| 0),
                ..Self::default()
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_launch_selects_the_welcome_note() {
        let docs = PathBuf::from("docs");
        let welcome = docs.join(docs::WELCOME_NOTE);
        let (first, session) =
            Session::initial(&ApplicationSettings::default(), Some(docs.clone()));

        assert!(first);
        assert_eq!(
            session,
            Session {
                vault: Some(docs),
                tabs: vec![welcome],
                active_tab_index: Some(0),
                ..Session::default()
            }
        );
        assert_eq!(
            Session::initial(&ApplicationSettings::default(), None),
            (true, Session::default())
        );
    }

    #[test]
    fn saved_workspace_takes_precedence_over_last_vault_and_docs() {
        let saved = Session {
            vault: Some("work".into()),
            tabs: vec!["work/notes.md".into()],
            active_tab_index: Some(0),
            expanded_folders: vec!["work/projects".into()],
            sidebar_selection: Some("work/notes.md".into()),
            ..Session::default()
        };
        let settings = ApplicationSettings {
            session: Some(saved.clone()),
            last_vault: Some("other".into()),
            ..ApplicationSettings::default()
        };

        assert_eq!(
            Session::initial(&settings, Some("docs".into())),
            (false, saved)
        );
    }

    #[test]
    fn intentionally_empty_workspace_stays_empty() {
        let settings = ApplicationSettings {
            session: Some(Session::default()),
            last_vault: Some("previous".into()),
            ..ApplicationSettings::default()
        };

        assert_eq!(
            Session::initial(&settings, Some("docs".into())),
            (false, Session::default())
        );
    }

    #[test]
    fn legacy_last_vault_is_restored_without_opening_docs() {
        let settings = ApplicationSettings {
            last_vault: Some("previous".into()),
            ..ApplicationSettings::default()
        };

        assert_eq!(
            Session::initial(&settings, Some("docs".into())),
            (
                false,
                Session {
                    vault: settings.last_vault,
                    ..Session::default()
                }
            )
        );
    }
}
