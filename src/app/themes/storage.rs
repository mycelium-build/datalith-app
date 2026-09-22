use std::{fs, io::Write as _, path::Path};

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};

use super::{ThemeDocument, ThemeEntry, ThemeLibrary};

const SCHEMA_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct StoredTheme {
    schema_version: u32,
    theme: ThemeDocument,
}

fn read(path: &Path) -> Result<ThemeDocument> {
    let stored: StoredTheme = serde_json::from_str(&fs::read_to_string(path)?)?;
    if stored.schema_version != SCHEMA_VERSION {
        bail!("Unsupported theme file version");
    }
    if stored.theme.name().trim().is_empty() {
        bail!("Theme name cannot be empty");
    }
    Ok(stored.theme)
}

impl ThemeLibrary {
    pub(super) fn load(&mut self) -> Vec<(String, anyhow::Error)> {
        let entries = match fs::read_dir(&self.directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
            Err(error) => return vec![("Custom themes".into(), error.into())],
        };
        let mut paths = Vec::new();
        let mut errors = Vec::new();
        for entry in entries {
            match entry {
                Ok(entry) if entry.path().extension().is_some_and(|ext| ext == "json") => {
                    paths.push(entry.path());
                }
                Ok(_) => {}
                Err(error) => errors.push(("Custom themes".into(), error.into())),
            }
        }
        paths.sort();
        for path in paths {
            let result = read(&path).and_then(|document| {
                if self
                    .entries
                    .keys()
                    .any(|name| name.eq_ignore_ascii_case(document.name()))
                {
                    bail!("A theme with this name already exists");
                }
                self.entries.insert(
                    document.name().to_owned(),
                    ThemeEntry {
                        document,
                        path: Some(path.clone()),
                    },
                );
                Ok(())
            });
            if let Err(error) = result {
                errors.push((path.display().to_string(), error));
            }
        }
        errors
    }
}

pub(super) fn save(
    library: &mut ThemeLibrary,
    document: &ThemeDocument,
    replacing: Option<&str>,
) -> Result<()> {
    if document.name().trim().is_empty() {
        bail!("Enter a theme name");
    }
    if library
        .entries
        .keys()
        .any(|name| name.eq_ignore_ascii_case(document.name()) && Some(name.as_str()) != replacing)
    {
        bail!("A theme with this name already exists. Choose another name.");
    }
    let mut document = document.clone();
    document.config.is_default = false;
    let existing_path = replacing
        .map(|name| {
            library
                .entries
                .get(name)
                .and_then(|entry| entry.path.clone())
                .context("Built-in themes must be saved as a new theme")
        })
        .transpose()?;
    fs::create_dir_all(&library.directory)?;
    // Names are display values, never paths. The random identity survives renames.
    let path = existing_path.unwrap_or_else(|| {
        library
            .directory
            .join(format!("theme-{:032x}.json", rand::random::<u128>()))
    });
    let temporary = library
        .directory
        .join(format!(".theme-{:032x}.tmp", rand::random::<u128>()));
    let result = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let stored = StoredTheme {
            schema_version: SCHEMA_VERSION,
            theme: document.clone(),
        };
        file.write_all(&serde_json::to_vec_pretty(&stored)?)?;
        file.sync_all()?;
        fs::rename(&temporary, &path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result?;
    if let Some(name) = replacing {
        library.entries.remove(name);
    }
    library.entries.insert(
        document.name().to_owned(),
        ThemeEntry {
            document,
            path: Some(path),
        },
    );
    Ok(())
}
