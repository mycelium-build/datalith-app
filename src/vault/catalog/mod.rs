use std::path::PathBuf;
use anyhow::{Context, Result, anyhow};

mod database;
use database::CatalogDatabase;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct WikiLinkEdge {
    pub(crate) source: PathBuf,
    pub(crate) target: PathBuf,
}

#[derive(Clone, Debug)]
pub(crate) struct CatalogDocument {
    pub(crate) path: PathBuf,
    pub(crate) metadata: Option<serde_json::Value>,
}

#[derive(Clone, Debug)]
pub(crate) struct DocumentSelection {
    pub(crate) documents: Vec<CatalogDocument>,
    pub(crate) exceeded_limit: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct LinkedDocumentSelection {
    pub(crate) documents: Vec<CatalogDocument>,
    pub(crate) links: Vec<WikiLinkEdge>,
    pub(crate) exceeded_limit: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct CatalogQuery {
    pub(crate) extension: Option<String>,
    pub(crate) filter: CatalogFilter,
    pub(crate) limit: usize,
}

#[derive(Clone, Debug)]
pub(crate) enum CatalogFilter {
    MatchAll,
    Compare {
        property: CatalogProperty,
        comparison: CatalogComparison,
        value: CatalogScalar,
    },
    Contains {
        property: CatalogProperty,
        value: CatalogScalar,
    },
    InFolder(String),
    And(Vec<CatalogFilter>),
    Or(Vec<CatalogFilter>),
    Not(Box<CatalogFilter>),
}

#[derive(Clone, Debug)]
pub(crate) enum CatalogProperty {
    Metadata(Vec<String>),
    File(CatalogFileField),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum CatalogFileField {
    Name,
    Extension,
    Path,
    Folder,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum CatalogComparison {
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
}

#[derive(Clone, Debug)]
pub(crate) enum CatalogScalar {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
}

#[derive(Clone)]
pub(crate) struct VaultCatalog {
    root: PathBuf,
    database: CatalogDatabase,
}

impl VaultCatalog {
    pub(crate) fn open(
        root: PathBuf,
        _file_types: crate::document::file_types::RegisteredFileTypes,
    ) -> Result<Self> {
        let database = pollster::block_on(CatalogDatabase::open(&root))?;
        Ok(Self { root, database })
    }

    #[must_use]
    pub(crate) fn root(&self) -> PathBuf {
        self.root.clone()
    }

    #[allow(dead_code)]
    pub(crate) async fn query_documents(&self, query: CatalogQuery) -> Result<DocumentSelection> {
        let database = self.database.clone();
        std::thread::Builder::new()
            .name("vault-catalog-query".into())
            .stack_size(8 * 1024 * 1024)
            .spawn(move || pollster::block_on(database.query_documents(query)))
            .context("Failed to start catalog query thread")?
            .join()
            .map_err(|_| anyhow!("Catalog query thread panicked"))?
    }

    pub(crate) async fn query_documents_with_links(
        &self,
        query: CatalogQuery,
    ) -> Result<LinkedDocumentSelection> {
        let database = self.database.clone();
        let (selection, stored_links) = std::thread::Builder::new()
            .name("vault-catalog-query".into())
            .stack_size(8 * 1024 * 1024)
            .spawn(move || pollster::block_on(database.query_documents_with_links(query)))
            .context("Failed to start catalog query thread")?
            .join()
            .map_err(|_| anyhow!("Catalog query thread panicked"))??;
        let links = stored_links
            .into_iter()
            .map(|link| WikiLinkEdge {
                source: link.source,
                target: link.target,
            })
            .collect();
        Ok(LinkedDocumentSelection {
            documents: selection.documents,
            links,
            exceeded_limit: selection.exceeded_limit,
        })
    }
}
