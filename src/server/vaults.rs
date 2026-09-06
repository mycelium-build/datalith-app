//! Vault listing and note writing: the domain logic behind the API handlers.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::error::ApiError;
use crate::document::markdown::build_note_document;
use crate::vault::file_ops;

/// A vault advertised to clients: its display name and absolute path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vault {
    pub name: String,
    pub path: PathBuf,
}

/// Writes a Markdown note with YAML frontmatter into the requested vault
/// and returns its path relative to the vault root.
pub fn write_note(
    vaults: &[Vault],
    vault_id: &str,
    name: &str,
    folder: Option<&str>,
    properties: &BTreeMap<String, String>,
    content: &str,
) -> Result<String, ApiError> {
    let vault = resolve_vault(vault_id, vaults)
        .ok_or_else(|| ApiError::not_found(format!("Unknown vault: {vault_id}")))?;

    let name = sanitize_name(name)?;
    let folder = sanitize_folder(folder)?;

    let property_list: Vec<(String, String)> = properties
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let document = build_note_document(&property_list, content);

    let directory = match &folder {
        Some(folder) => {
            let directory = vault.path.join(folder);
            std::fs::create_dir_all(&directory)
                .map_err(|error| ApiError::internal(format!("Failed to create folder: {error}")))?;
            directory
        }
        None => vault.path.clone(),
    };

    let file_name = if name.to_ascii_lowercase().ends_with(".md") {
        name
    } else {
        format!("{name}.md")
    };
    let target = file_ops::unique_name(&directory, &file_name);
    std::fs::write(&target, document)
        .map_err(|error| ApiError::internal(format!("Failed to write note: {error}")))?;

    let relative = target
        .strip_prefix(&vault.path)
        .unwrap_or(&target)
        .to_string_lossy()
        .replace('\\', "/");
    Ok(relative)
}

fn resolve_vault<'a>(vault_id: &str, vaults: &'a [Vault]) -> Option<&'a Vault> {
    vaults
        .iter()
        .find(|vault| vault.path.to_string_lossy() == vault_id)
        .or_else(|| vaults.iter().find(|vault| vault.name == vault_id))
}

fn munge_name_characters(raw: &str) -> String {
    raw.chars()
        .map(|c| match c {
            c if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*') => ' ',
            c => c,
        })
        .collect()
}

fn sanitize_name(raw: &str) -> Result<String, ApiError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request("Note name must not be empty"));
    }
    if trimmed == "." || trimmed == ".." || trimmed.contains('/') || trimmed.contains('\\') {
        return Err(ApiError::bad_request(
            "Note name must not contain path separators",
        ));
    }
    let cleaned = munge_name_characters(trimmed)
        .trim()
        .trim_start_matches('.')
        .trim_end_matches(['.', ' '])
        .trim()
        .to_owned();
    if cleaned.is_empty() {
        return Err(ApiError::bad_request(
            "Note name must contain visible characters",
        ));
    }
    Ok(cleaned)
}

fn sanitize_folder(raw: Option<&str>) -> Result<Option<PathBuf>, ApiError> {
    let Some(raw) = raw.map(str::trim).filter(|folder| !folder.is_empty()) else {
        return Ok(None);
    };
    if raw.starts_with('/') || raw.starts_with('\\') || Path::new(raw).is_absolute() {
        return Err(ApiError::bad_request("Folder must be a relative path"));
    }
    let mut chars = raw.chars();
    let first = chars.next();
    let second = chars.next();
    if second == Some(':') && first.is_some_and(|c| c.is_ascii_alphabetic()) {
        return Err(ApiError::bad_request("Folder must be a relative path"));
    }
    let mut folder = PathBuf::new();
    for segment in raw.split(['/', '\\']) {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        if segment == "." || segment == ".." {
            return Err(ApiError::bad_request(
                "Folder must not contain '.' or '..' segments",
            ));
        }
        if segment.starts_with('.') {
            return Err(ApiError::bad_request(
                "Folder must not contain hidden folders",
            ));
        }
        let cleaned = munge_name_characters(segment).trim().to_owned();
        if !cleaned.is_empty() {
            folder.push(cleaned);
        }
    }
    Ok((!folder.as_os_str().is_empty()).then_some(folder))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_reject_separators_and_traversal() {
        for bad in ["", "   ", "..", ".", "a/b", "a\\b", " ./x "] {
            assert!(sanitize_name(bad).is_err(), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn names_munge_invalid_characters() {
        assert_eq!(sanitize_name("My: Note?").unwrap(), "My  Note");
        assert_eq!(sanitize_name("  Spaced  ").unwrap(), "Spaced");
        assert_eq!(sanitize_name("Note.md").unwrap(), "Note.md");
        assert_eq!(sanitize_name("...hidden").unwrap(), "hidden");
        assert_eq!(sanitize_name("trailing. ").unwrap(), "trailing");
    }

    #[test]
    fn folders_reject_traversal_absolutes_and_hidden() {
        for bad in [
            Some("../evil"),
            Some("a/../b"),
            Some("/absolute"),
            Some("C:\\tmp"),
            Some(".hidden"),
            Some("notes/.secret"),
        ] {
            assert!(sanitize_folder(bad).is_err(), "{bad:?} must be rejected");
        }
        assert_eq!(sanitize_folder(None).unwrap(), None);
        assert_eq!(sanitize_folder(Some("")).unwrap(), None);
        assert_eq!(
            sanitize_folder(Some("Clips/Web")).unwrap(),
            Some(PathBuf::from("Clips").join("Web"))
        );
    }
}
