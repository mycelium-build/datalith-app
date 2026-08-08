#![allow(dead_code)]
#![allow(unused_imports)]

mod fixtures;

// Mirror the crate's module structure so `crate::` paths resolve correctly.
mod document {
    pub mod file_types;
}
#[path = "../src/vault/mod.rs"]
mod vault;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use document::file_types::{FileTypeCapabilities, RegisteredFileTypes};
use fixtures::generator::{VaultConfig, generate_vault};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use vault::catalog::VaultCatalog;

fn md_file_types() -> RegisteredFileTypes {
    RegisteredFileTypes::new([(
        "md".into(),
        FileTypeCapabilities {
            text_search: true,
            wiki_links: true,
            yaml_frontmatter: true,
        },
    )])
}

fn vault_root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("datalith-bench-{}-{}", name, std::process::id()))
}

fn delete_catalog_db(root: &Path) {
    let db = root.join(".datalith/catalog.db");
    let _ = fs::remove_file(&db);
    let _ = fs::remove_file(db.with_extension("db-wal"));
    let _ = fs::remove_file(db.with_extension("db-shm"));
}

fn open_catalog(root: PathBuf) -> VaultCatalog {
    // A benchmark cannot proceed if the catalog fails to open, so abort loudly.
    #[allow(clippy::expect_used)]
    VaultCatalog::open(root, md_file_types()).expect("failed to open vault catalog for benchmark")
}

fn bench_cold_start(c: &mut Criterion) {
    let mut group = c.benchmark_group("cold_start");
    group.sample_size(10);

    for &n in &[500, 1000, 3000] {
        let root = vault_root(&format!("cold-{n}"));
        let config = VaultConfig {
            projects: n / 50,
            daily_notes: n / 3,
            notes: n,
            references: n / 10,
            seed: 42,
        };
        let _ = fs::remove_dir_all(&root);
        generate_vault(&root, &config);

        group.bench_function(format!("{n}_files"), |b| {
            b.iter_batched(
                || delete_catalog_db(&root),
                |()| {
                    let catalog = open_catalog(root.clone());
                    catalog.wait_until_ready(Duration::from_mins(2));
                    std::hint::black_box(&catalog);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_warm_start(c: &mut Criterion) {
    let mut group = c.benchmark_group("warm_start");
    group.sample_size(10);

    for &n in &[500, 1000, 3000] {
        let root = vault_root(&format!("warm-{n}"));
        let config = VaultConfig {
            projects: n / 50,
            daily_notes: n / 3,
            notes: n,
            references: n / 10,
            seed: 42,
        };
        let _ = fs::remove_dir_all(&root);
        generate_vault(&root, &config);

        {
            let catalog = open_catalog(root.clone());
            catalog.wait_until_ready(Duration::from_mins(2));
        }

        group.bench_function(format!("{n}_files"), |b| {
            b.iter(|| {
                let catalog = open_catalog(root.clone());
                catalog.wait_until_ready(Duration::from_mins(2));
                std::hint::black_box(&catalog);
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_cold_start, bench_warm_start);
criterion_main!(benches);
