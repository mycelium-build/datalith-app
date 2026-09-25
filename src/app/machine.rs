//! Stable app-derived identity used to isolate each machine's workspace.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::OnceLock;

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MACHINE_ID_SCHEMA_VERSION: u32 = 1;
const MACHINE_ID_FILE: &str = "machine-id.json";

/// Return this installation's stable, app-derived machine identity.
///
/// The OS identifier is hashed before it is written locally or used in a vault
/// path. If the OS has no supported stable identifier, a random value is
/// persisted locally and reused on later calls.
pub fn machine_id() -> Result<String> {
    static MACHINE_ID: OnceLock<String> = OnceLock::new();
    if let Some(machine_id) = MACHINE_ID.get() {
        return Ok(machine_id.clone());
    }
    let machine_id = load_or_create_machine_id(&identity_data_dir())?;
    let _ = MACHINE_ID.set(machine_id.clone());
    Ok(MACHINE_ID.get().cloned().unwrap_or(machine_id))
}

#[cfg(test)]
fn identity_data_dir() -> std::path::PathBuf {
    static TEST_DATA_DIR: OnceLock<std::path::PathBuf> = OnceLock::new();
    TEST_DATA_DIR
        .get_or_init(|| {
            let path = std::env::temp_dir()
                .join(format!("datalith-test-machine-id-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            path
        })
        .clone()
}

#[cfg(not(test))]
fn identity_data_dir() -> std::path::PathBuf {
    super::data_dir()
}

fn load_or_create_machine_id(data_dir: &Path) -> Result<String> {
    load_or_create_machine_id_with(data_dir, os_machine_identifier)
}

fn load_or_create_machine_id_with(
    data_dir: &Path,
    os_identity: impl FnOnce() -> Result<String>,
) -> Result<String> {
    let file = data_dir.join(MACHINE_ID_FILE);
    if let Some(machine_id) = read_machine_id(&file)? {
        return Ok(machine_id);
    }
    fs::create_dir_all(data_dir).with_context(|| {
        format!(
            "Failed to create application data directory: {}",
            data_dir.display()
        )
    })?;

    let machine_id = os_identity().map_or_else(
        |_| derive_machine_id(&format!("fallback:{:032x}", rand::random::<u128>())),
        |identifier| derive_machine_id(&identifier),
    );
    let stored = StoredMachineId {
        schema_version: MACHINE_ID_SCHEMA_VERSION,
        machine_id: machine_id.clone(),
    };
    let json = serde_json::to_vec(&stored).context("Failed to serialize machine identity")?;
    match OpenOptions::new().write(true).create_new(true).open(&file) {
        Ok(mut local_file) => {
            if let Err(error) = local_file
                .write_all(&json)
                .and_then(|()| local_file.sync_all())
            {
                let _ = fs::remove_file(&file);
                return Err(error).with_context(|| {
                    format!("Failed to persist machine identity: {}", file.display())
                });
            }
            Ok(machine_id)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            read_machine_id(&file)?.context("Machine identity was created but could not be read")
        }
        Err(error) => Err(error)
            .with_context(|| format!("Failed to persist machine identity: {}", file.display())),
    }
}

fn read_machine_id(path: &Path) -> Result<Option<String>> {
    let json = match fs::read_to_string(path) {
        Ok(json) => json,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("Failed to read machine identity: {}", path.display()));
        }
    };
    let stored: StoredMachineId = serde_json::from_str(&json)
        .with_context(|| format!("Failed to parse machine identity: {}", path.display()))?;
    ensure!(
        stored.schema_version == MACHINE_ID_SCHEMA_VERSION,
        "Unsupported machine identity schema version {} in {}",
        stored.schema_version,
        path.display()
    );
    ensure!(
        stored.machine_id.len() == 64
            && stored
                .machine_id
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit()),
        "Invalid machine identity in {}",
        path.display()
    );
    Ok(Some(stored.machine_id))
}

#[derive(Deserialize, Serialize)]
struct StoredMachineId {
    schema_version: u32,
    machine_id: String,
}

fn derive_machine_id(identifier: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"com.mycelium-build.datalith.machine-id.v1\0");
    digest.update(identifier.trim().as_bytes());
    format!("{:x}", digest.finalize())
}

#[cfg(target_os = "linux")]
fn os_machine_identifier() -> Result<String> {
    for path in ["/etc/machine-id", "/var/lib/dbus/machine-id"] {
        if let Ok(identifier) = fs::read_to_string(path) {
            let identifier = identifier.trim();
            if !identifier.is_empty() {
                return Ok(identifier.to_owned());
            }
        }
    }
    bail!("The operating system has no readable machine identity")
}

#[cfg(target_os = "macos")]
fn os_machine_identifier() -> Result<String> {
    let output = std::process::Command::new("/usr/sbin/ioreg")
        .args(["-rd1", "-c", "IOPlatformExpertDevice"])
        .output()
        .context("Failed to query the macOS platform identity")?;
    ensure!(
        output.status.success(),
        "macOS platform identity query failed"
    );
    let output = String::from_utf8_lossy(&output.stdout);
    output
        .lines()
        .find(|line| line.contains("IOPlatformUUID"))
        .and_then(|line| line.rsplit('"').nth(1))
        .map(str::trim)
        .filter(|identifier| !identifier.is_empty())
        .map(ToOwned::to_owned)
        .context("macOS platform identity was unavailable")
}

#[cfg(target_os = "windows")]
fn os_machine_identifier() -> Result<String> {
    let output = std::process::Command::new("reg")
        .args([
            "query",
            "HKLM\\SOFTWARE\\Microsoft\\Cryptography",
            "/v",
            "MachineGuid",
        ])
        .output()
        .context("Failed to query the Windows platform identity")?;
    ensure!(
        output.status.success(),
        "Windows platform identity query failed"
    );
    let output = String::from_utf8_lossy(&output.stdout);
    output
        .lines()
        .find(|line| line.contains("MachineGuid"))
        .and_then(|line| line.split_whitespace().last())
        .map(str::trim)
        .filter(|identifier| !identifier.is_empty())
        .map(ToOwned::to_owned)
        .context("Windows platform identity was unavailable")
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn os_machine_identifier() -> Result<String> {
    bail!("The operating system has no supported stable machine identity")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "datalith-machine-id-{}-{:032x}",
            std::process::id(),
            rand::random::<u128>()
        ));
        fs::create_dir_all(&root).expect("create temporary identity root");
        root
    }

    #[test]
    fn app_hash_is_stable_and_fallback_identity_is_persisted() {
        let raw_os_id = "raw-os-machine-id";
        let derived = derive_machine_id(raw_os_id);
        assert_eq!(derived, derive_machine_id(raw_os_id));
        assert_ne!(derived, raw_os_id);
        assert_eq!(derived.len(), 64);

        let root = temp_root();
        let first = load_or_create_machine_id_with(&root, || bail!("OS ID unavailable"))
            .expect("persist fallback identity");
        let second = load_or_create_machine_id_with(&root, || {
            Ok("different OS identity must not replace persisted value".to_owned())
        })
        .expect("reuse saved identity");
        assert_eq!(first, second);
        let contents = fs::read_to_string(root.join(MACHINE_ID_FILE)).expect("read local id");
        assert!(!contents.contains(raw_os_id));

        let _ = fs::remove_dir_all(root);
    }
}
