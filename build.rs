#[cfg(target_os = "windows")]
fn main() -> Result<(), embed_resource::CompilationResult> {
    validate_channel();
    build_commit();
    updater_public_key();
    let channel = include_str!("CHANNEL");
    let directory = format!("assets/logo/{}", channel.trim());
    println!("cargo:rerun-if-changed={directory}/datalith.ico");
    println!("cargo:rerun-if-changed={directory}/datalith.rc");
    embed_resource::compile(format!("{directory}/datalith.rc"), embed_resource::NONE)
        .manifest_required()
}

#[cfg(not(target_os = "windows"))]
fn main() {
    validate_channel();
    build_commit();
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
