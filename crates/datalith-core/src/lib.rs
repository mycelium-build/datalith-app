//! Datalith document formats, independent of GPUI and of a vault location.
//!
//! This crate interprets document content: Markdown and its frontmatter, Base definitions and
//! their expressions, the rules for reading and rewriting wiki links, and the file types Datalith
//! recognizes with their capabilities. Applying these rules to files on disk belongs to
//! `datalith-vault`.
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

pub mod base;
pub mod expr;
pub mod file_types;
pub mod filter;
pub mod links;
pub mod markdown;
