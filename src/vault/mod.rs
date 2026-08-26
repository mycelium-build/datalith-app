pub mod catalog;
pub mod file_ops;
pub mod links;
pub mod path;
pub mod search;

pub const DATALITH_DIR_NAME: &str = ".datalith";

pub use catalog::{BaseDocument, BaseQuery, CatalogEvent, CatalogState, SummaryRef, VaultCatalog};
