use std::path::PathBuf;
use std::time::Duration;

use cargo_packager_updater::semver::Version;
use cargo_packager_updater::url::Url;
use cargo_packager_updater::{Config, Update, UpdaterBuilder};

use super::installation::STABLE_MANIFEST_ENDPOINT;
use super::source::{StagedUpdate, UpdateSource};
use super::state::UpdateFailure;

/// Total request timeout for discovery and download.
const REQUEST_TIMEOUT: Duration = Duration::from_mins(30);

pub struct CargoPackagerSource {
    endpoint: Url,
    pubkey: String,
    current_version: Version,
    executable: Option<PathBuf>,
}

impl CargoPackagerSource {
    /// Builds the production source for the stable channel.
    pub fn stable(pubkey: &str) -> anyhow::Result<Self> {
        Self::new(
            STABLE_MANIFEST_ENDPOINT,
            pubkey,
            crate::app::version::version(),
        )
    }

    /// Builds a source against an explicit endpoint and version.
    pub fn new(endpoint: &str, pubkey: &str, version: &str) -> anyhow::Result<Self> {
        Ok(Self {
            endpoint: endpoint.parse()?,
            pubkey: pubkey.to_string(),
            current_version: version.parse()?,
            executable: None,
        })
    }

    /// Overrides the executable path the crate updates.
    #[cfg(test)]
    fn with_executable(mut self, path: PathBuf) -> Self {
        self.executable = Some(path);
        self
    }

    fn build_updater(&self) -> Result<cargo_packager_updater::Updater, UpdateFailure> {
        let mut builder = UpdaterBuilder::new(
            self.current_version.clone(),
            Config {
                endpoints: vec![self.endpoint.clone()],
                pubkey: self.pubkey.clone(),
                windows: None,
            },
        )
        .timeout(REQUEST_TIMEOUT);
        if let Some(executable) = &self.executable {
            builder = builder.executable_path(executable);
        }
        builder.build().map_err(|error| {
            eprintln!("Update discovery could not be configured: {error}");
            UpdateFailure::Check
        })
    }
}

impl UpdateSource for CargoPackagerSource {
    fn check(&self) -> Result<Option<Box<dyn StagedUpdate>>, UpdateFailure> {
        let updater = self.build_updater()?;
        match updater.check() {
            Ok(Some(update)) => {
                let version = update.version.parse().map_err(|error| {
                    eprintln!("Update manifest announced an invalid version: {error}");
                    UpdateFailure::Check
                })?;
                Ok(Some(Box::new(CargoStagedUpdate { update, version })))
            }
            Ok(None) => Ok(None),
            Err(error) => {
                eprintln!("Update discovery failed: {error}");
                Err(UpdateFailure::Check)
            }
        }
    }
}

struct CargoStagedUpdate {
    update: Update,
    version: Version,
}

impl StagedUpdate for CargoStagedUpdate {
    fn version(&self) -> &Version {
        &self.version
    }

    fn download(&self, on_progress: &dyn Fn(u64, Option<u64>)) -> Result<Vec<u8>, UpdateFailure> {
        let received = std::cell::Cell::new(0_u64);
        self.update
            .download_extended(
                |chunk, total| {
                    // The crate reports chunk lengths, including a final zero-byte read.
                    received.set(
                        received
                            .get()
                            .saturating_add(u64::try_from(chunk).unwrap_or(u64::MAX)),
                    );
                    on_progress(received.get(), total);
                },
                || {},
            )
            .map_err(|error| {
                eprintln!("Update download failed: {error}");
                UpdateFailure::Download
            })
    }

    fn install(&self, bytes: &[u8]) -> Result<(), UpdateFailure> {
        self.update.install(bytes.to_vec()).map_err(|error| {
            eprintln!("Update install failed: {error}");
            UpdateFailure::Install
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io::{BufRead, BufReader, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use cargo_packager_updater::semver::Version;

    use super::*;
    use base64::Engine as _;

    fn encode_public_key(pk: &minisign::PublicKey) -> String {
        base64::engine::general_purpose::STANDARD
            .encode(pk.to_box().expect("public key box").to_string())
    }

    const BUNDLE: &[u8] = &[42; 128 * 1024];

    struct TestServer {
        addr: std::net::SocketAddr,
        routes: Arc<Mutex<HashMap<String, Vec<u8>>>>,
        running: Arc<AtomicBool>,
        handle: Option<thread::JoinHandle<()>>,
    }

    impl TestServer {
        fn start() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
            listener.set_nonblocking(true).expect("nonblocking");
            let addr = listener.local_addr().expect("addr");
            let routes = Arc::new(Mutex::new(HashMap::new()));
            let running = Arc::new(AtomicBool::new(true));
            let routes_thread = Arc::clone(&routes);
            let running_thread = Arc::clone(&running);
            let handle = thread::spawn(move || {
                while running_thread.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, _)) => serve(stream, &routes_thread),
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(_) => break,
                    }
                }
            });
            Self {
                addr,
                routes,
                running,
                handle: Some(handle),
            }
        }

        fn set(&self, path: &str, body: Vec<u8>) {
            self.routes
                .lock()
                .expect("routes lock")
                .insert(path.to_string(), body);
        }

        fn url(&self, path: &str) -> String {
            format!("http://{}{path}", self.addr)
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            self.running.store(false, Ordering::SeqCst);
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    fn serve(mut stream: TcpStream, routes: &Arc<Mutex<HashMap<String, Vec<u8>>>>) {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let mut reader = BufReader::new(stream.try_clone().expect("clone"));
        let mut request_line = String::new();
        if reader.read_line(&mut request_line).is_err() {
            return;
        }
        let path = request_line
            .split_whitespace()
            .nth(1)
            .unwrap_or("/")
            .to_string();
        loop {
            let mut header = String::new();
            match reader.read_line(&mut header) {
                Ok(0) | Err(_) => break,
                Ok(_) if header == "\r\n" => break,
                Ok(_) => {}
            }
        }

        let body = routes.lock().expect("routes lock").get(&path).cloned();
        let status = if body.is_some() { 200 } else { 404 };
        let body = body.unwrap_or_default();
        let reason = if status == 200 { "OK" } else { "Not Found" };
        let head = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(head.as_bytes());
        let _ = stream.write_all(&body);
        let _ = stream.flush();
    }

    fn keypair() -> minisign::KeyPair {
        minisign::KeyPair::generate_unencrypted_keypair().expect("keypair")
    }

    fn sign(keypair: &minisign::KeyPair, bytes: &[u8]) -> String {
        let signature = minisign::sign(None, &keypair.sk, bytes, None, None).expect("sign");
        base64::engine::general_purpose::STANDARD.encode(signature.to_string())
    }

    fn manifest(url: &str, signature: &str, version: &str) -> Vec<u8> {
        let platform = serde_json::json!({
            "signature": signature,
            "url": url,
            "format": "appimage",
        });
        serde_json::to_vec(&serde_json::json!({
            "version": version,
            "pub_date": "2026-09-11T00:00:00Z",
            "platforms": {
                "linux-x86_64": platform,
                "linux-aarch64": platform,
                "macos-x86_64": platform,
                "macos-aarch64": platform,
                "windows-x86_64": platform,
                "windows-aarch64": platform,
            }
        }))
        .expect("manifest json")
    }

    fn source(server: &TestServer, pubkey: &str, target: &std::path::Path) -> CargoPackagerSource {
        CargoPackagerSource::new(&server.url("/manifest.json"), pubkey, "0.1.0")
            .expect("source")
            .with_executable(target.to_path_buf())
    }

    fn temp_target(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "datalith-update-{name}-{}-{}",
            std::process::id(),
            std::thread::current()
                .name()
                .unwrap_or("test")
                .replace("::", "-")
        ));
        std::fs::write(&path, b"old appimage").expect("write target");
        path
    }

    #[test]
    fn signed_bundle_downloads_with_cumulative_progress() {
        let server = TestServer::start();
        let keypair = keypair();
        let pubkey = encode_public_key(&keypair.pk);
        let signature = sign(&keypair, BUNDLE);
        server.set("/bundle", BUNDLE.to_vec());
        server.set(
            "/manifest.json",
            manifest(&server.url("/bundle"), &signature, "0.2.0"),
        );
        let target = temp_target("ready");
        let source = source(&server, &pubkey, &target);

        let update = source.check().expect("check").expect("update");
        assert_eq!(update.version(), &Version::new(0, 2, 0));

        let seen = std::sync::Mutex::new(Vec::new());
        let bytes = update
            .download(&|received, total| {
                seen.lock().unwrap().push((received, total));
            })
            .expect("download");
        assert_eq!(bytes, BUNDLE);
        let seen = seen.into_inner().unwrap();
        assert!(!seen.is_empty(), "progress callback fired");
        assert_eq!(
            seen.first().and_then(|(_, total)| *total),
            Some(u64::try_from(BUNDLE.len()).expect("bundle length fits in u64"))
        );

        assert_eq!(
            seen.last().map(|(received, _)| *received),
            Some(u64::try_from(BUNDLE.len()).expect("bundle length"))
        );
        assert!(seen.windows(2).all(|pair| pair[0].0 <= pair[1].0));
        #[cfg(target_os = "linux")]
        {
            update.install(&bytes).expect("install");
            assert_eq!(std::fs::read(&target).expect("read target"), BUNDLE);
        }
        let _ = std::fs::remove_file(&target);
    }

    #[test]
    fn tampered_bundle_is_rejected() {
        let server = TestServer::start();
        let keypair = keypair();
        let pubkey = encode_public_key(&keypair.pk);
        let signature = sign(&keypair, BUNDLE);
        let mut tampered = BUNDLE.to_vec();
        tampered.extend_from_slice(b"tampered");
        server.set("/bundle", tampered);
        server.set(
            "/manifest.json",
            manifest(&server.url("/bundle"), &signature, "0.2.0"),
        );
        let target = temp_target("tampered");
        let source = source(&server, &pubkey, &target);

        let update = source.check().expect("check").expect("update");
        assert_eq!(update.download(&|_, _| {}), Err(UpdateFailure::Download));
        let _ = std::fs::remove_file(&target);
    }

    #[test]
    fn a_bundle_signed_by_another_key_is_rejected() {
        let server = TestServer::start();
        let trusted = keypair();
        let attacker = keypair();
        let pubkey = encode_public_key(&trusted.pk);
        let signature = sign(&attacker, BUNDLE);
        server.set("/bundle", BUNDLE.to_vec());
        server.set(
            "/manifest.json",
            manifest(&server.url("/bundle"), &signature, "0.2.0"),
        );
        let target = temp_target("wrong-key");
        let source = source(&server, &pubkey, &target);

        let update = source.check().expect("check").expect("update");
        assert_eq!(update.download(&|_, _| {}), Err(UpdateFailure::Download));
        let _ = std::fs::remove_file(&target);
    }

    #[test]
    fn a_missing_signature_is_rejected() {
        let server = TestServer::start();
        let keypair = keypair();
        let pubkey = encode_public_key(&keypair.pk);
        server.set("/bundle", BUNDLE.to_vec());
        server.set(
            "/manifest.json",
            manifest(&server.url("/bundle"), "", "0.2.0"),
        );
        let target = temp_target("unsigned");
        let source = source(&server, &pubkey, &target);

        let update = source.check().expect("check").expect("update");
        assert_eq!(update.download(&|_, _| {}), Err(UpdateFailure::Download));
        let _ = std::fs::remove_file(&target);
    }

    #[test]
    fn an_equal_version_offers_no_update() {
        let server = TestServer::start();
        let keypair = keypair();
        let pubkey = encode_public_key(&keypair.pk);
        let signature = sign(&keypair, BUNDLE);
        server.set("/bundle", BUNDLE.to_vec());
        server.set(
            "/manifest.json",
            manifest(&server.url("/bundle"), &signature, "0.1.0"),
        );
        let target = temp_target("equal");
        let source = source(&server, &pubkey, &target);

        let result = source.check().expect("check");
        assert!(result.is_none());
        let _ = std::fs::remove_file(&target);
    }

    #[test]
    fn a_missing_manifest_is_a_check_failure() {
        let server = TestServer::start();
        let keypair = keypair();
        let pubkey = encode_public_key(&keypair.pk);
        let target = temp_target("missing");
        let source = source(&server, &pubkey, &target);

        assert!(matches!(source.check(), Err(UpdateFailure::Check)));
        let _ = std::fs::remove_file(&target);
    }
}
