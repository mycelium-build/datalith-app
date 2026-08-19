mod fixtures;

// Mirror the crate's module structure so `crate::` paths resolve correctly.
#[allow(dead_code, unused_imports)]
mod document {
    pub mod file_types;
}
#[allow(dead_code, unused_imports)]
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

fn reset_catalog_state(root: &Path) {
    let path = root.join(".datalith");
    remove_benchmark_dir(&path);
}

fn remove_benchmark_dir(path: &Path) {
    if let Err(error) = fs::remove_dir_all(path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        eprintln!(
            "Failed to remove benchmark directory {}: {error}",
            path.display()
        );
    }
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
        remove_benchmark_dir(&root);
        generate_vault(&root, &config);

        group.bench_function(format!("{n}_files"), |b| {
            b.iter_batched(
                || reset_catalog_state(&root),
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
        remove_benchmark_dir(&root);
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
