//! Routes shared by the desktop and mobile URL handlers.
use percent_encoding::percent_decode_str;
use std::path::{Component, Path, PathBuf};
const SCHEME_PREFIX: &str = "datalith://";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeepLink {
    Launch,
    OpenNote { vault: String, path: String },
}

#[must_use]
pub fn parse(url: &str) -> Option<DeepLink> {
    let remainder = url.strip_prefix(SCHEME_PREFIX)?;
    let (route, query) = remainder.split_once('?').unwrap_or((remainder, ""));
    match route {
        "launch" => Some(DeepLink::Launch),
        "open" => {
            let mut vault = None;
            let mut path = None;
            for pair in query.split('&') {
                let Some((key, value)) = pair.split_once('=') else {
                    continue;
                };
                let decoded = percent_decode_str(value).decode_utf8_lossy().into_owned();
                match key {
                    "vault" => vault = Some(decoded),
                    "path" => path = Some(decoded),
                    _ => {}
                }
            }
            Some(DeepLink::OpenNote {
                vault: vault?,
                path: path?,
            })
        }
        _ => None,
    }
}

/// Resolves a vault-relative note path inside `root`, rejecting anything that escapes the vault.
#[must_use]
pub fn resolve_in_vault(root: &Path, relative: &str) -> Option<PathBuf> {
    let candidate = PathBuf::from(relative.replace('\\', "/"));
    if candidate.is_absolute()
        || candidate.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return None;
    }
    Some(root.join(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_launch_and_open_routes() {
        assert_eq!(parse("datalith://launch"), Some(DeepLink::Launch));
        assert_eq!(
            parse("datalith://open?vault=Notes&path=Clips%2FMy%20Note.md"),
            Some(DeepLink::OpenNote {
                vault: "Notes".into(),
                path: "Clips/My Note.md".into()
            })
        );
        assert_eq!(
            parse("datalith://open?vault=My%20Vault&path=Note.md"),
            Some(DeepLink::OpenNote {
                vault: "My Vault".into(),
                path: "Note.md".into()
            })
        );
        assert_eq!(parse("datalith://open"), None);
        assert_eq!(parse("datalith://open?path=Clips%2FMy%20Note.md"), None);
        assert_eq!(parse("datalith://open?vault=Notes"), None);
        assert_eq!(parse("datalith://unknown"), None);
        assert_eq!(parse("https://example.com"), None);
    }

    #[test]
    fn resolve_keeps_paths_inside_the_vault() {
        let root = Path::new("/vault");
        assert_eq!(
            resolve_in_vault(root, "Clips/Note.md"),
            Some(PathBuf::from("/vault/Clips/Note.md"))
        );
        assert_eq!(resolve_in_vault(root, "/etc/passwd"), None);
        assert_eq!(resolve_in_vault(root, "../outside.md"), None);
        assert_eq!(resolve_in_vault(root, "a/../../b.md"), None);
    }
}
