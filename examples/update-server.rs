use std::io::{BufRead as _, BufReader, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use base64::{Engine as _, engine::general_purpose::STANDARD};

struct Fixture {
    directory: PathBuf,
    endpoint: String,
    signature: String,
    bundle: Vec<u8>,
}

fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let endpoint = format!("http://{}", listener.local_addr()?);
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/update-ui");
    let (fixture, _) = prepare(directory, endpoint.clone())?;
    println!("Serving signed fake updates at {endpoint}");
    let launch = if cfg!(windows) {
        "powershell -ExecutionPolicy Bypass -File target/update-ui/run.ps1"
    } else {
        "sh target/update-ui/run.sh"
    };
    println!("In another terminal, from the repository root:\n\n  {launch}\n");
    println!(
        "Scenarios: ready, unknown-length, up-to-date, check-error, download-error, bad-signature."
    );
    println!(
        "Restart to update quits and writes {} without reopening Datalith.",
        fixture.directory.join("applied").display()
    );
    let fixture = Arc::new(fixture);
    for connection in listener.incoming() {
        let stream = connection?;
        let fixture = Arc::clone(&fixture);
        std::thread::Builder::new()
            .name("update-fixture-http".into())
            .spawn(move || {
                if let Err(error) = serve(stream, &fixture) {
                    eprintln!("HTTP request ended: {error}");
                }
            })?;
    }
    Ok(())
}

fn prepare(directory: PathBuf, endpoint: String) -> anyhow::Result<(Fixture, String)> {
    std::fs::create_dir_all(&directory)?;
    let target = directory.join("Datalith.AppImage");
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::write(&target, "#!/bin/sh\nexit 0\n")?;
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755))?;
    }
    let marker = directory.join("applied");
    if marker.exists() {
        std::fs::remove_file(&marker)?;
    }
    let bundle = prepare_bundle(&directory, &marker)?;
    let keys = minisign::KeyPair::generate_unencrypted_keypair()?;
    let pubkey = STANDARD.encode(keys.pk.to_box()?.to_string());
    let signature =
        STANDARD.encode(minisign::sign(None, &keys.sk, bundle.as_slice(), None, None)?.to_string());
    std::fs::write(directory.join("scenario"), "ready\n")?;
    write_launcher(&directory, &endpoint, &pubkey, &target)?;
    Ok((
        Fixture {
            directory,
            endpoint,
            signature,
            bundle,
        },
        pubkey,
    ))
}

fn prepare_bundle(directory: &Path, marker: &Path) -> anyhow::Result<Vec<u8>> {
    let source = directory.join("fake_update.rs");
    std::fs::write(
        &source,
        format!(
            "fn main() -> std::io::Result<()> {{ std::fs::write({:?}, b\"Fake update applied\\n\") }}",
            marker.to_string_lossy(),
        ),
    )?;
    let executable = directory.join(if cfg!(windows) {
        "fake-update.exe"
    } else {
        "fake-update"
    });
    anyhow::ensure!(
        Command::new("rustc")
            .arg(&source)
            .args(["-o"])
            .arg(&executable)
            .status()?
            .success(),
        "Could not compile the fake update"
    );
    if cfg!(target_os = "macos") {
        let app = directory.join("payload/Datalith.app/Contents");
        std::fs::create_dir_all(app.join("MacOS"))?;
        std::fs::copy(&executable, app.join("MacOS/datalith"))?;
        std::fs::write(
            app.join("Info.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleExecutable</key><string>datalith</string><key>CFBundleIdentifier</key><string>build.mycelium.datalith.update-test</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>"#,
        )?;
        let archive = directory.join("update.app.tar.gz");
        anyhow::ensure!(
            Command::new("tar")
                // AppleDouble sidecars break the updater's app-root stripping.
                .env("COPYFILE_DISABLE", "1")
                .arg("-czf")
                .arg(&archive)
                .arg("-C")
                .arg(directory.join("payload"))
                .arg("Datalith.app")
                .status()?
                .success(),
            "Could not archive the fake app"
        );
        Ok(std::fs::read(archive)?)
    } else {
        Ok(std::fs::read(executable)?)
    }
}

fn write_launcher(
    directory: &Path,
    endpoint: &str,
    pubkey: &str,
    target: &Path,
) -> anyhow::Result<()> {
    if cfg!(windows) {
        std::fs::write(
            directory.join("run.ps1"),
            format!(
                "$ErrorActionPreference = 'Stop'\nSet-Location -LiteralPath $PSScriptRoot\nSet-Location ../..\n$channel = Get-Content -Raw -LiteralPath CHANNEL\ntry {{\nSet-Content -NoNewline -Encoding Ascii -LiteralPath CHANNEL -Value stable\n$env:DATALITH_RELEASE_TAG = 'v0.1.0'\n$env:DATALITH_UPDATER_PUBKEY = '{pubkey}'\n$env:DATALITH_UPDATE_ENDPOINT = '{endpoint}/manifest.json'\ncargo build --locked --bin datalith\nif ($LASTEXITCODE -ne 0) {{ throw 'Build failed' }}\n}} finally {{ Set-Content -NoNewline -Encoding Ascii -LiteralPath CHANNEL -Value $channel }}\n& target/debug/datalith.exe\n"
            ),
        )?;
    } else {
        let launch = if cfg!(target_os = "macos") {
            "mkdir -p target/update-ui/installed\ncp -R target/update-ui/payload/Datalith.app target/update-ui/installed/\ncp target/debug/datalith target/update-ui/installed/Datalith.app/Contents/MacOS/datalith\nexec target/update-ui/installed/Datalith.app/Contents/MacOS/datalith"
        } else {
            "exec target/debug/datalith"
        };
        let environment = format!(
            "#!/bin/sh\nset -eu\ncd {}\nexport DATALITH_RELEASE_TAG=v0.1.0\nexport DATALITH_UPDATER_PUBKEY={}\nexport DATALITH_UPDATE_ENDPOINT={}/manifest.json\nchannel=$(cat CHANNEL)\ntrap 'printf \"%s\\n\" \"$channel\" > CHANNEL' EXIT\nprintf 'stable\\n' > CHANNEL\ncargo build --locked --bin datalith\nprintf '%s\\n' \"$channel\" > CHANNEL\ntrap - EXIT\n",
            quote(env!("CARGO_MANIFEST_DIR")),
            quote(pubkey),
            endpoint,
        );
        let installation = if cfg!(target_os = "linux") {
            format!("export APPIMAGE={}\n", quote(&target.to_string_lossy()))
        } else {
            "unset APPIMAGE\n".into()
        };
        std::fs::write(
            directory.join("run.sh"),
            format!("{environment}{installation}{launch}\n"),
        )?;
        if cfg!(target_os = "linux") {
            std::fs::write(
                directory.join("run-package.sh"),
                format!("{environment}unset APPIMAGE APPDIR\nexec target/debug/datalith\n"),
            )?;
        }
    }
    Ok(())
}

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn serve(mut stream: TcpStream, fixture: &Fixture) -> anyhow::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut request = String::new();
    let mut reader = BufReader::new(&stream);
    reader.read_line(&mut request)?;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header == "\r\n" {
            break;
        }
    }
    let path = request
        .split_whitespace()
        .nth(1)
        .context("missing request path")?;
    let scenario = std::fs::read_to_string(fixture.directory.join("scenario"))?;
    let scenario = scenario.trim();
    println!("{path} ({scenario})");
    match path {
        "/manifest.json" if scenario == "check-error" => reply(&mut stream, 500, b"check failed"),
        "/manifest.json" => {
            let version = if scenario == "up-to-date" {
                "0.1.0"
            } else {
                "0.2.0"
            };
            let platform = serde_json::json!({
                "url": format!("{}/bundle", fixture.endpoint),
                "signature": fixture.signature,
                "format": match std::env::consts::OS {
                    "macos" => "app",
                    "windows" => "nsis",
                    _ => "appimage",
                },
            });
            let manifest = serde_json::to_vec(&serde_json::json!({
                "version": version,
                "pub_date": "2026-09-11T00:00:00Z",
                "platforms": { format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH): platform },
            }))?;
            reply(&mut stream, 200, &manifest)
        }
        "/bundle" if scenario == "download-error" => reply(&mut stream, 500, b"download failed"),
        "/bundle" => {
            write!(stream, "HTTP/1.1 200 OK\r\nConnection: close\r\n")?;
            if scenario != "unknown-length" {
                write!(stream, "Content-Length: {}\r\n", fixture.bundle.len())?;
            }
            write!(stream, "\r\n")?;
            let mut bundle = fixture.bundle.clone();
            if scenario == "bad-signature"
                && let Some(byte) = bundle.last_mut()
            {
                *byte ^= 1;
            }
            for chunk in bundle.chunks(bundle.len().div_ceil(32).max(1)) {
                stream.write_all(chunk)?;
                stream.flush()?;
                std::thread::sleep(Duration::from_millis(250));
            }
            Ok(())
        }
        _ => reply(&mut stream, 404, b"not found"),
    }
}

fn reply(stream: &mut TcpStream, status: u16, body: &[u8]) -> anyhow::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status} Response\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_http_fixture_verifies_and_runs_the_fake_update() -> anyhow::Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let directory =
            std::env::temp_dir().join(format!("datalith-update-fixture-{}", std::process::id()));
        #[cfg(not(target_os = "macos"))]
        let target = directory.join("Datalith.AppImage");
        #[cfg(target_os = "macos")]
        let target = {
            let payload = directory.join("payload/Datalith.app");
            std::fs::create_dir_all(&payload)?;
            // Force tar to encounter metadata that would produce ._Datalith.app.
            anyhow::ensure!(
                Command::new("xattr")
                    .args(["-w", "com.datalith.update-test", "fixture metadata"])
                    .arg(&payload)
                    .status()?
                    .success(),
                "Could not add fixture metadata"
            );
            let target = directory.join("installed/Datalith.app/Contents/MacOS/datalith");
            std::fs::create_dir_all(target.parent().context("missing executable parent")?)?;
            std::fs::write(&target, "old executable")?;
            target
        };
        let (fixture, pubkey) = prepare(directory.clone(), endpoint.clone())?;
        let expected = fixture.bundle.clone();
        let server = std::thread::spawn(move || -> anyhow::Result<()> {
            for connection in listener.incoming().take(2) {
                serve(connection?, &fixture)?;
            }
            Ok(())
        });
        let updater = cargo_packager_updater::UpdaterBuilder::new(
            "0.1.0".parse()?,
            cargo_packager_updater::Config {
                endpoints: vec![format!("{endpoint}/manifest.json").parse()?],
                pubkey,
                windows: None,
            },
        )
        .executable_path(&target)
        .timeout(Duration::from_secs(15))
        .build()?;
        let update = updater.check()?.context("expected fake update")?;
        let bytes = update.download_extended(|_, _| {}, || {})?;
        assert_eq!(bytes, expected);
        #[cfg(target_os = "linux")]
        {
            assert_eq!(std::fs::read_to_string(&target)?, "#!/bin/sh\nexit 0\n");
            update.install(bytes)?;
            assert!(Command::new(&target).status()?.success());
        }
        #[cfg(target_os = "macos")]
        {
            update.install(bytes)?;
            assert!(Command::new(&target).status()?.success());
        }
        #[cfg(target_os = "windows")]
        assert!(
            Command::new(directory.join(format!("fake-update{}", std::env::consts::EXE_SUFFIX)))
                .status()?
                .success()
        );
        assert_eq!(
            std::fs::read_to_string(directory.join("applied"))?,
            "Fake update applied\n"
        );
        server
            .join()
            .map_err(|_| anyhow::anyhow!("HTTP fixture panicked"))??;
        std::fs::remove_dir_all(directory)?;
        Ok(())
    }
}
