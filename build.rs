#[cfg(target_os = "windows")]
fn main() -> Result<(), embed_resource::CompilationResult> {
    updater_public_key();
    println!("cargo:rerun-if-changed=assets/logo/datalith.ico");
    println!("cargo:rerun-if-changed=assets/logo/datalith.rc");

    embed_resource::compile("assets/logo/datalith.rc", embed_resource::NONE).manifest_required()
}

#[cfg(not(target_os = "windows"))]
fn main() {
    updater_public_key();
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
