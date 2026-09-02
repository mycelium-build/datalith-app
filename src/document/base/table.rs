//! Table view configuration.

use crate::document::base::{TableRowHeight, ViewKind};

/// The particular settings of a `type: table` view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TableConfig {
    pub row_height: TableRowHeight,
}

pub fn build(row_height: Option<TableRowHeight>) -> ViewKind {
    ViewKind::Table(TableConfig {
        row_height: row_height.unwrap_or_default(),
    })
}
