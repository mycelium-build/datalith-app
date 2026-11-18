//! Shared Datalith document editors and viewers.
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

pub mod actions;
pub mod contexts;
pub mod editors;
pub mod handler;
pub mod icons;
pub mod notifications;
pub mod registry;
pub mod scroll;
pub mod theme_scope;
mod todo_txt;
pub mod viewers;

pub mod fonts;
pub const BASE_FONT_SIZE: f32 = 16.0;
const LINE_HEIGHT: f32 = 1.6;
