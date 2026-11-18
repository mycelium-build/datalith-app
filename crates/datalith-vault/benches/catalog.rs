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

mod fixtures;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use datalith_core::file_types::{FileTypeCapabilities, RegisteredFileTypes};
use datalith_vault::{VaultCatalog, environment};
use fixtures::generator::{VaultConfig, generate_vault};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

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
    let path = environment::vault_dir(root).unwrap();
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

fn wait_until_ready(catalog: &VaultCatalog) {
    let start = std::time::Instant::now();
    while catalog.state() == datalith_vault::CatalogState::Syncing {
        assert!(
            start.elapsed() <= Duration::from_mins(2),
            "catalog sync timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn open_catalog(root: &Path) -> VaultCatalog {
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
                    let catalog = open_catalog(&root);
                    wait_until_ready(&catalog);
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
            let catalog = open_catalog(&root);
            wait_until_ready(&catalog);
        }

        group.bench_function(format!("{n}_files"), |b| {
            b.iter(|| {
                let catalog = open_catalog(&root);
                wait_until_ready(&catalog);
                std::hint::black_box(&catalog);
            });
        });
    }
    group.finish();
}

fn bench_catalog(c: &mut Criterion) {
    environment::init(environment::VaultEnvironment {
        embedded_vaults: &[],
        channel_directory: None,
        app_data_dir: std::env::temp_dir,
    });
    bench_cold_start(c);
    bench_warm_start(c);
}

criterion_group!(benches, bench_catalog);
criterion_main!(benches);
