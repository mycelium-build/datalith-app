#[cfg(target_os = "windows")]
fn main() -> Result<(), embed_resource::CompilationResult> {
    println!("cargo:rerun-if-changed=assets/icons/datalith.ico");
    println!("cargo:rerun-if-changed=assets/icons/datalith.rc");

    embed_resource::compile("assets/icons/datalith.rc", embed_resource::NONE)
        .manifest_required()
}

#[cfg(not(target_os = "windows"))]
fn main() {}
