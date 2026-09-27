fn main() -> Result<(), Box<dyn std::error::Error>> {
    validate_channel();
    build_commit();
    updater_public_key();
    bundle_docs()?;
    #[cfg(target_os = "windows")]
    {
        let channel = include_str!("CHANNEL");
        let directory = format!("assets/logo/{}", channel.trim());
        println!("cargo:rerun-if-changed={directory}/datalith.ico");
        println!("cargo:rerun-if-changed={directory}/datalith.rc");
        embed_resource::compile(format!("{directory}/datalith.rc"), embed_resource::NONE)
            .manifest_required()
            .map_err(|error| format!("Failed to embed Windows resources: {error:?}"))?;
    }
    Ok(())
}

/// Embed the documentation so installed binaries do not need the build checkout.
fn bundle_docs() -> Result<(), Box<dyn std::error::Error>> {
    use std::fmt::Write as _;
    use std::path::Path;

    fn collect(
        root: &Path,
        dir: &Path,
        output: &mut String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut entries = std::fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                collect(root, &path, output)?;
            } else {
                let relative = path
                    .strip_prefix(root)?
                    .to_string_lossy()
                    .replace('\\', "/");
                writeln!(
                    output,
                    "({relative:?}, include_bytes!({:?})),",
                    path.to_string_lossy()
                )?;
            }
        }
        Ok(())
    }

    println!("cargo:rerun-if-changed=docs/vault");
    let root = Path::new(&std::env::var("CARGO_MANIFEST_DIR")?).join("docs/vault");
    let mut output = String::from("const DOCS_FILES: &[(&str, &[u8])] = &[\n");
    collect(&root, &root, &mut output)?;
    output.push_str("];\n");
    std::fs::write(
        Path::new(&std::env::var("OUT_DIR")?).join("docs_vault.rs"),
        output,
    )?;
    Ok(())
}

fn updater_public_key() {
    println!("cargo:rerun-if-changed=scripts/updates/public-key.txt");
    println!("cargo:rerun-if-env-changed=DATALITH_UPDATER_PUBKEY");
    let key = include_str!("scripts/updates/public-key.txt").trim();
    let key = if std::env::var("PROFILE").as_deref() == Ok("debug") {
        std::env::var("DATALITH_UPDATER_PUBKEY").unwrap_or_else(|_| key.to_owned())
    } else {
        key.to_owned()
    };
    println!("cargo:rustc-env=DATALITH_UPDATER_PUBKEY={key}");
}

fn validate_channel() {
    println!("cargo:rerun-if-changed=CHANNEL");
    let channel = include_str!("CHANNEL");
    assert!(
        matches!(channel.trim(), "stable" | "preview" | "dev"),
        "invalid CHANNEL"
    );
}

fn git(args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn build_commit() {
    let commit = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=DATALITH_COMMIT={commit}");
    let reference = git(&["symbolic-ref", "-q", "HEAD"]);
    for name in [Some("HEAD"), Some("packed-refs"), reference.as_deref()]
        .into_iter()
        .flatten()
    {
        if let Some(path) = git(&["rev-parse", "--git-path", name])
            && std::path::Path::new(&path).exists()
        {
            println!("cargo:rerun-if-changed={path}");
        }
    }
}
