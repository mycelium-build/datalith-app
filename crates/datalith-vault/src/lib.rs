//! Datalith vault access: applies `datalith-core` rules to the files of a concrete vault.
//!
//! A vault is a folder of documents, or a read-only collection embedded in the application.
//! This crate reads and writes its files, resolves links across documents, maintains the
//! queryable catalog and the full-text index, and answers searches. The application declares
//! its embedded vaults and channel layout once with [`environment::init`].
#![deny(warnings)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::indexing_slicing,
        clippy::string_slice
    )
)]

pub mod catalog;
pub mod draft;
pub mod environment;
pub mod file_ops;
pub mod path;
pub mod search;
pub mod source;
#[cfg(any(test, feature = "test-support"))]
pub mod testing;

/// Name of the folder at the root of a directory vault that holds Datalith's own state.
pub const DATALITH_DIR_NAME: &str = ".datalith";

pub use catalog::{BaseDocument, BaseQuery, CatalogEvent, CatalogState, SummaryRef, VaultCatalog};
