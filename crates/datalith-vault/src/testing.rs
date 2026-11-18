//! A vault environment for tests, with a read-only vault made of a few documentation files.
//!
//! The vault's own tests and, through the `test-support` feature, the tests of crates that
//! build on it install this environment automatically. Applications install their own with
//! [`crate::environment::init`].

use std::path::PathBuf;

use crate::environment::VaultEnvironment;
use crate::source::EmbeddedVault;

const FILES: &[(&str, &[u8])] = &[
    ("Basics.md", include_bytes!("../../../docs/vault/Basics.md")),
    (
        "Overview.base",
        include_bytes!("../../../docs/vault/Overview.base"),
    ),
    (
        "Tour.todotxt",
        include_bytes!("../../../docs/vault/Tour.todotxt"),
    ),
    (
        "Welcome.md",
        include_bytes!("../../../docs/vault/Welcome.md"),
    ),
    (
        "formats/Markdown.md",
        include_bytes!("../../../docs/vault/formats/Markdown.md"),
    ),
];

/// A read-only vault with the identity of the embedded documentation and some of its files.
pub const DOCUMENTATION: EmbeddedVault =
    EmbeddedVault::new("documentation", "Datalith Documentation", FILES);

static EMBEDDED_VAULTS: [EmbeddedVault; 1] = [DOCUMENTATION];

/// The test environment: [`DOCUMENTATION`] as the only embedded vault, the stable `.datalith`
/// layout and a temporary application data directory for each test thread.
#[must_use]
pub fn environment() -> VaultEnvironment {
    VaultEnvironment {
        embedded_vaults: &EMBEDDED_VAULTS,
        channel_directory: None,
        app_data_dir,
    }
}

fn app_data_dir() -> PathBuf {
    let test_name = std::thread::current()
        .name()
        .unwrap_or("test")
        .replace("::", "-");
    std::env::temp_dir().join(format!(
        "datalith-test-cache-{}-{test_name}",
        std::process::id()
    ))
}
