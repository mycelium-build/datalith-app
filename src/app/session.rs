//! The workspace restored when the application opens again.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{docs, settings::ApplicationSettings};

/// A saved workspace. `Some(Session::default())` is an intentionally empty session.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct Session {
    pub(crate) vault: Option<PathBuf>,
    pub(crate) tabs: Vec<PathBuf>,
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
                active_tab,
                ..Self::default()
            },
        )
    }
}
