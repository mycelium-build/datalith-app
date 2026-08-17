#[cfg(target_os = "windows")]
fn main() -> Result<(), embed_resource::CompilationResult> {
    println!("cargo:rerun-if-changed=assets/logo/datalith.ico");
    println!("cargo:rerun-if-changed=assets/logo/datalith.rc");

    embed_resource::compile("assets/logo/datalith.rc", embed_resource::NONE).manifest_required()
}

#[cfg(not(target_os = "windows"))]
fn main() {}
