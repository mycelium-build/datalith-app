//! Executes compiled Base query plans against the catalog database.

use anyhow::Result;
use turso::Value;

use crate::vault::catalog::{BaseQuery, BaseSelection};

use super::CatalogDatabase;

mod compiler;
mod execute;
mod summaries;

impl CatalogDatabase {
    pub(crate) async fn query_base(&self, query: BaseQuery) -> Result<BaseSelection> {
        let connection = self.read_connection()?;
        connection.execute("BEGIN DEFERRED", ()).await?;
        let result = self.query_base_on(&connection, query).await;
        let _ = connection.execute("ROLLBACK", ()).await;
        result
    }
}

fn turso_to_json(value: Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Integer(value) => serde_json::Value::from(value),
        Value::Real(value) => serde_json::Value::from(value),
        // SQLite has no array type:
        // json_extract over a list property returns its JSON serialization as text.
        // Decode it so every list key projects as a real array.
        Value::Text(value) => {
            let trimmed = value.trim_start();
            if trimmed.starts_with('[') || trimmed.starts_with('{') {
                serde_json::from_str(&value).unwrap_or(serde_json::Value::String(value))
            } else {
                serde_json::Value::String(value)
            }
        }
        Value::Blob(value) => {
            serde_json::Value::String(value.iter().fold(String::new(), |mut out, byte| {
                use std::fmt::Write as _;
                let _ = write!(out, "{byte:02x}");
                out
            }))
        }
    }
}

fn parenthesize(sql: &str) -> String {
    format!("({sql})")
}

#[cfg(test)]
pub(super) mod test_support {
    use std::fs;

    use crate::document::file_types::{FileTypeCapabilities, RegisteredFileTypes};
    use crate::document::filter::Filter;

    pub(super) fn file_types() -> RegisteredFileTypes {
        RegisteredFileTypes::new([
            (
                "md".into(),
                FileTypeCapabilities {
                    text_search: true,
                    wiki_links: true,
                    yaml_frontmatter: true,
                },
            ),
            (
                "png".into(),
                FileTypeCapabilities {
                    text_search: false,
                    wiki_links: false,
                    yaml_frontmatter: false,
                },
            ),
        ])
    }

    pub(super) async fn catalog_with(
        files: &[(&str, &str)],
    ) -> (super::super::CatalogDatabase, std::path::PathBuf) {
        let unique: u64 = files
            .iter()
            .map(|(name, content)| {
                let seed = u64::try_from(name.len()).unwrap_or_default();
                let size = u64::try_from(content.len()).unwrap_or_default();
                seed.wrapping_mul(31)
                    .wrapping_add(size)
                    .wrapping_mul(2_654_435_761)
            })
            .fold(
                files.len().try_into().unwrap_or(u64::MAX),
                u64::wrapping_add,
            );
        let root = std::env::temp_dir().join(format!(
            "datalith-base-query-{}-{unique}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let mut changed = Vec::new();
        for (name, content) in files {
            let path = root.join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&path, content).unwrap();
            changed.push(path);
        }
        let database = super::super::CatalogDatabase::open(&root).await.unwrap();
        database
            .synchronize(&[], &changed, &file_types())
            .await
            .unwrap();
        (database, root)
    }

    pub(super) fn expression(source: &str) -> Filter {
        Filter::Expression(
            crate::document::filter::Expression::parse(source)
                .unwrap_or_else(|error| panic!("test setup: {error}")),
        )
    }
}

#[cfg(test)]
mod tests {

    use super::test_support::{catalog_with, expression};
    use super::*;
    use crate::document::base::SortDirection;
    use crate::document::filter::Filter;
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;

    #[allow(dead_code)]
    fn source(text: &str) -> String {
        text.to_string()
    }

    /// The kitchen-sink plan exercising every compiler arm at once:
    /// folder/text/NOT/GLOB/modulo filters, formulas, grouping, all summary families, and graph classes.
    fn worst_case_query() -> BaseQuery {
        let mut formulas = BTreeMap::new();
        formulas.insert(
            "completion".to_string(),
            crate::document::expr::Expr::parse("(progress / pages * 100).round(0)").unwrap(),
        );
        BaseQuery {
            filters: Filter::And(vec![
                expression("file.inFolder(\"Notes\")"),
                expression("price > 5"),
                expression("status == \"done\" || !title.startsWith(\"Draft\")"),
                expression("!file.hasTag(\"hidden\")"),
                expression("rating % 2 == 1"),
            ]),
            formulas,
            projections: vec![
                source("title"),
                source("author"),
                source("status"),
                source("rating"),
                source("formula.completion"),
                source("file.tags"),
                source("file.links"),
            ],
            sort: vec![(source("rating"), SortDirection::Desc)],
            group_by: Some((source("status"), SortDirection::Asc)),
            summaries: vec![
                (
                    source("pages"),
                    crate::vault::catalog::SummaryRef::Default(crate::document::base::Summary::Sum),
                ),
                (
                    source("rating"),
                    crate::vault::catalog::SummaryRef::Default(
                        crate::document::base::Summary::Median,
                    ),
                ),
                (
                    source("rating"),
                    crate::vault::catalog::SummaryRef::Default(
                        crate::document::base::Summary::Average,
                    ),
                ),
                (
                    source("rating"),
                    crate::vault::catalog::SummaryRef::Default(
                        crate::document::base::Summary::Stddev,
                    ),
                ),
            ],
            classes: vec![expression("file.hasTag(\"x\")")],
            limit: Some(100),
        }
    }

    /// batch path resolution is the statement shape that crashed the app
    #[test]
    fn batch_resolution_compiles_on_a_2mib_stack() {
        let (database, root) = pollster::block_on(catalog_with(&[
            ("Notes/A.md", "---\n---\n"),
            ("Notes/B.md", "---\n---\n"),
        ]));
        let expected_root = root.clone();
        let handle = std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(move || {
                pollster::block_on(async {
                    let targets: BTreeSet<String> =
                        ["A", "Notes/A.md", "Missing.png", "Notes/Missing", "B"]
                            .iter()
                            .map(|target| (*target).to_string())
                            .collect();
                    let resolved = database
                        .resolve_paths(targets)
                        .await
                        .expect("batch resolve must compile on a 2 MiB stack");
                    assert_eq!(
                        resolved.get("A"),
                        Some(&Some(expected_root.join("Notes/A.md"))),
                        "unqualified stem resolves to the note"
                    );
                    assert_eq!(
                        resolved.get("Notes/A.md"),
                        Some(&Some(expected_root.join("Notes/A.md"))),
                        "qualified path resolves directly"
                    );
                    assert_eq!(resolved.get("Missing.png"), Some(&None));
                    assert_eq!(resolved.get("Notes/Missing"), Some(&None));
                    assert_eq!(
                        resolved.get("B"),
                        Some(&Some(expected_root.join("Notes/B.md")))
                    );
                    drop(database);
                    let _ = fs::remove_dir_all(root);
                });
            })
            .unwrap();
        handle.join().unwrap();
    }

    /// turso's debug-frame footprint makes this the tightest workable bound.
    #[test]
    fn worst_case_queries_compile_on_a_small_stack() {
        let handle = std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(move || {
                pollster::block_on(async {
                    let (database, root) = catalog_with(&[
                        (
                            "Notes/A.md",
                            "---\ntitle: Alpha\nprice: 7\nstatus: done\nrating: 1\nprogress: 50\npages: 100\ntags: [x]\n---\n",
                        ),
                        (
                            "Notes/B.md",
                            "---\ntitle: Draft B\nprice: 30\nstatus: open\nrating: 2\nprogress: 80\npages: 300\n---\n",
                        ),
                        (
                            "Notes/C.md",
                            "---\nprice: 9\nstatus: done\nrating: 3\nprogress: 25\npages: 200\n---\n",
                        ),
                    ])
                    .await;
                    let selection = database
                        .query_base(worst_case_query())
                        .await
                        .expect("query must compile on a 2 MiB stack");
                    assert_eq!(selection.total_matched, 2, "A and C match; Draft B is excluded");
                    assert!(selection.documents[0].path.ends_with("C.md"));
                    assert!(selection.documents[1].path.ends_with("A.md"));
                    assert_eq!(selection.documents[1].values[4].as_f64(), Some(50.0));
                    let whole_set = selection
                        .summaries
                        .iter()
                        .find(|(key, _)| key.is_none())
                        .expect("whole-set summary entry");
                    assert_eq!(whole_set.1[0].as_i64(), Some(300));
                    assert_eq!(whole_set.1[1].as_f64(), Some(2.0));
                    assert_eq!(selection.documents[0].class_hits, vec![false]);
                    (database, root)
                })
            })
            .unwrap();
        let (_database, root) = handle.join().unwrap();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn filters_sorts_and_projects_expressions() {
        pollster::block_on(async {
            let (database, root) = catalog_with(&[
                (
                    "A.md",
                    "---\nprice: 10\nstatus: done\ntags: [rust]\n---\n[[B]]",
                ),
                ("B.md", "---\nprice: 30\nstatus: open\n---\n"),
                ("C.md", "---\nprice: 20\nstatus: done\n---\n[[B]]"),
            ])
            .await;
            let query = BaseQuery {
                filters: Filter::And(vec![
                    expression("price > 5"),
                    expression("status == \"done\" || status == \"open\""),
                ]),
                formulas: BTreeMap::new(),
                projections: vec![source("price"), source("status"), source("file.name")],
                sort: vec![(source("price"), SortDirection::Asc)],
                group_by: None,
                summaries: Vec::new(),
                classes: Vec::new(),
                limit: Some(50),
            };
            let selection = database.query_base(query).await.unwrap();
            assert_eq!(selection.total_matched, 3);
            // Sorted by price ascending: A(10), C(20), B(30).
            assert!(selection.documents[0].path.ends_with("A.md"));
            assert!(selection.documents[2].path.ends_with("B.md"));
            assert_eq!(selection.documents[0].values[0].as_i64(), Some(10));
            assert_eq!(selection.documents[0].values[2], serde_json::json!("A"));
            drop(database);
            let _ = fs::remove_dir_all(root);
        });
    }

    #[test]
    fn new_file_properties_project_from_catalog_tables() {
        pollster::block_on(async {
            let (database, root) = catalog_with(&[
                (
                    "Source.md",
                    "---\ntags: [front, body-tag]\n---\n[[Target]] ![[Pic.png]]",
                ),
                ("Target.md", "---\n---\nback at [[Source]]"),
                ("Pic.png", "fake image bytes"),
            ])
            .await;
            let query = BaseQuery {
                filters: Filter::MatchAll,
                formulas: BTreeMap::new(),
                projections: vec![
                    source("file.tags"),
                    source("file.links"),
                    source("file.embeds"),
                    source("file.backlinks"),
                    source("file.properties"),
                ],
                sort: vec![],
                group_by: None,
                summaries: Vec::new(),
                classes: Vec::new(),
                limit: None,
            };
            let selection = database.query_base(query).await.unwrap();
            let source_doc = selection
                .documents
                .iter()
                .find(|document| document.path.ends_with("Source.md"))
                .unwrap();
            let tags = &source_doc.values[0];
            assert_eq!(
                tags,
                &serde_json::json!(["front", "body-tag"]),
                "tags project as a plain array: {tags}"
            );
            let links = source_doc.values[1].as_str().unwrap_or_default();
            assert!(links.contains("Target.md"), "{links}");
            let embeds = source_doc.values[2].as_str().unwrap_or_default();
            assert!(embeds.contains("Pic.png"), "{embeds}");
            let backlinks = source_doc.values[3].as_str().unwrap_or_default();
            assert!(backlinks.contains("Target.md"), "{backlinks}");
            drop(database);
            let _ = fs::remove_dir_all(root);
        });
    }

    #[test]
    fn graph_class_filters_project_as_boolean_columns() {
        pollster::block_on(async {
            let (database, root) = catalog_with(&[
                ("Projects/Alpha.md", "---\ntags: [project]\n---\n[[Beta]]"),
                ("Reading/Beta.md", "---\ntags: [book]\n---\n"),
            ])
            .await;
            let query = BaseQuery {
                filters: Filter::MatchAll,
                formulas: BTreeMap::new(),
                projections: vec![],
                sort: vec![],
                group_by: None,
                summaries: Vec::new(),
                classes: vec![
                    expression("file.hasTag(\"project\")"),
                    expression("file.inFolder(\"Reading\")"),
                ],
                limit: Some(50),
            };
            let selection = database.query_base(query).await.unwrap();
            assert_eq!(selection.documents.len(), 2);
            let alpha = selection
                .documents
                .iter()
                .find(|document| document.path.ends_with("Alpha.md"))
                .unwrap();
            let beta = selection
                .documents
                .iter()
                .find(|document| document.path.ends_with("Beta.md"))
                .unwrap();
            assert_eq!(alpha.class_hits, vec![true, false]);
            assert_eq!(beta.class_hits, vec![false, true]);
            // Links still arrive for edge construction.
            assert_eq!(alpha.links.len(), 1);
            drop(database);
            let _ = fs::remove_dir_all(root);
        });
    }
}

#[cfg(test)]
mod example_tests {
    use std::fs;
    use std::path::PathBuf;

    use super::test_support::file_types;
    use super::*;
    use crate::document::base::BaseDefinition;
    use crate::vault::catalog::BaseQuery;

    /// The shipped documentation example must stay valid and queryable.
    #[test]
    fn documented_example_parses_and_queries() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let source = fs::read_to_string(manifest.join("docs/vault/examples/bases/Library.base"))
            .expect("test setup: read Library.base");
        let definition = BaseDefinition::parse(&source)
            .unwrap_or_else(|error| panic!("documented example must parse: {error}"));

        let root =
            std::env::temp_dir().join(format!("datalith-example-base-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        // Mirror the documented vault layout so file.inFolder("examples/bases/notes")
        // matches exactly as it does for users.
        let notes = manifest.join("docs/vault/examples/bases/notes");
        let destination_root = root.join("examples/bases/notes");
        fs::create_dir_all(&destination_root).unwrap();
        copy_dir(&notes, &notes, &destination_root);
        let changed: Vec<PathBuf> = walk_files(&root);

        pollster::block_on(async move {
            let database = CatalogDatabase::open(&root).await.unwrap();
            database
                .synchronize(&[], &changed, &file_types())
                .await
                .unwrap();
            for view in &definition.views {
                let mut projections: Vec<String> =
                    view.order.iter().map(|p| p.source.clone()).collect();
                for rule in &view.sort {
                    if !projections.contains(&rule.source) {
                        projections.push(rule.source.clone());
                    }
                }
                if let Some(group) = &view.group_by
                    && !projections.contains(&group.property.source)
                {
                    projections.push(group.property.source.clone());
                }
                for source in view.summaries.keys() {
                    if !projections.contains(source) {
                        projections.push(source.clone());
                    }
                }
                let summaries = view
                    .summaries
                    .iter()
                    .map(|(source, name)| {
                        (
                            source.clone(),
                            definition
                                .resolve_summary_ref(name)
                                .unwrap_or_else(|| panic!("summary {name} must resolve")),
                        )
                    })
                    .collect();
                let query = BaseQuery {
                    filters: definition.combined_filters(view),
                    formulas: definition.formulas.clone(),
                    projections,
                    sort: view
                        .sort
                        .iter()
                        .map(|rule| (rule.source.clone(), rule.direction))
                        .collect(),
                    group_by: view
                        .group_by
                        .as_ref()
                        .map(|group| (group.property.source.clone(), group.direction)),
                    summaries,
                    limit: Some(100),
                    classes: Vec::new(),
                };
                let selection = database
                    .query_base(query)
                    .await
                    .unwrap_or_else(|error| panic!("view {:?} must query: {error}", view.name));
                assert!(
                    !selection.documents.is_empty(),
                    "view {:?} returned no rows",
                    view.name
                );
            }
            drop(database);
            let _ = fs::remove_dir_all(root);
        });
    }

    fn copy_dir(base: &PathBuf, dir: &PathBuf, into: &PathBuf) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                copy_dir(base, &path, into);
            } else {
                let relative = path.strip_prefix(base).unwrap();
                let destination = into.join(relative);
                fs::create_dir_all(destination.parent().unwrap()).unwrap();
                fs::copy(&path, destination).unwrap();
            }
        }
    }

    fn walk_files(root: &std::path::Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|extension| extension == "md") {
                    files.push(path);
                }
            }
        }
        files
    }
}
