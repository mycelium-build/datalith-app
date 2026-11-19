pub fn bundle_docs() -> Result<(), Box<dyn std::error::Error>> {
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

    println!("cargo:rerun-if-changed=../../docs/vault");
    let root = Path::new(&std::env::var("CARGO_MANIFEST_DIR")?).join("../../docs/vault");
    let mut output = String::from("const DOCS_FILES: &[(&str, &[u8])] = &[\n");
    collect(&root, &root, &mut output)?;
    output.push_str("];\n");
    std::fs::write(
        Path::new(&std::env::var("OUT_DIR")?).join("docs_vault.rs"),
        output,
    )?;
    Ok(())
}
