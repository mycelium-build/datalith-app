//! HTTP-level integration tests:
//! each test drives the `OpenAPI` application through poem's `TestClient`,
//! plus lifecycle tests on the real manager.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use poem::http::Method;
use poem::http::StatusCode;
use poem::test::TestClient;
use serde_json::{Value, json};

use super::ServerConfig;
use super::ServerManager;
use super::ServerStatus;
use super::api::ApiContext;
use super::api::MAX_BODY_BYTES;
use super::build_app;
use super::vaults::Vault;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_id() -> u64 {
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// A temporary vault directory that removes itself on drop.
struct TempVault(PathBuf);

impl TempVault {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "datalith-server-{tag}-{}-{}",
            std::process::id(),
            unique_id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl std::ops::Deref for TempVault {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for TempVault {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn vault(dir: &TempVault) -> Vault {
    Vault {
        name: dir.0.file_name().unwrap().to_string_lossy().into_owned(),
        path: dir.0.clone(),
    }
}

fn context(token: Option<&str>, vaults: Vec<Vault>) -> ApiContext {
    ApiContext::new(token.map(str::to_owned), Arc::new(move || vaults.clone()))
}

fn app(token: Option<&str>, vaults: Vec<Vault>) -> impl poem::Endpoint + 'static {
    build_app(context(token, vaults), 0)
}

#[tokio::test]
async fn ping_reports_app_and_version() {
    let dir = TempVault::new("ping");
    let client = TestClient::new(app(None, vec![vault(&dir)]));
    let response = client.get("/ping").send().await;
    response.assert_status_is_ok();
    response
        .assert_json(json!({
            "app": "datalith",
            "version": env!("CARGO_PKG_VERSION")
        }))
        .await;
}

#[tokio::test]
async fn token_guards_every_endpoint() {
    let dir = TempVault::new("auth");
    let client = TestClient::new(app(Some("s3cret"), vec![vault(&dir)]));
    for path in ["/ping", "/api/vaults"] {
        let denied = client.get(path).send().await;
        denied.assert_status(StatusCode::UNAUTHORIZED);
        let allowed = client
            .get(path)
            .header("Authorization", "Bearer s3cret")
            .send()
            .await;
        allowed.assert_status_is_ok();
    }
    let denied_save = client
        .post("/api/notes")
        .header("Content-Type", "application/json")
        .body(json!({"vault": "v", "name": "n", "properties": {}, "content": ""}).to_string())
        .send()
        .await;
    denied_save.assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn vaults_lists_context_vaults() {
    let dir = TempVault::new("vaults");
    let entry = vault(&dir);
    let expected = json!([{
        "name": entry.name,
        "path": entry.path.to_string_lossy(),
    }]);
    let client = TestClient::new(app(None, vec![entry]));
    let response = client.get("/api/vaults").send().await;
    response.assert_status_is_ok();
    response.assert_json(expected).await;
}

#[tokio::test]
async fn save_note_writes_frontmatter_and_reports_relative_path() {
    let dir = TempVault::new("save");
    let name = vault(&dir).name;
    let client = TestClient::new(app(None, vec![vault(&dir)]));
    let response = client
        .post("/api/notes")
        .header("Content-Type", "application/json")
        .body(
            json!({
                "vault": name,
                "name": "Hello",
                "folder": "Clips",
                "properties": {
                    "source": "https://example.com/article",
                    "tags": "clips",
                },
                "content": "Body text."
            })
            .to_string(),
        )
        .send()
        .await;
    response.assert_status_is_ok();
    response
        .assert_json(json!({ "path": "Clips/Hello.md" }))
        .await;

    let file = std::fs::read_to_string(dir.join("Clips").join("Hello.md")).unwrap();
    assert!(file.contains("source: \"https://example.com/article\""));
    assert!(file.contains("tags: \"clips\""));
    assert!(file.trim_end().ends_with("Body text."));
}

#[tokio::test]
async fn save_note_numbers_colliding_names() {
    let dir = TempVault::new("collision");
    let name = vault(&dir).name;
    let client = TestClient::new(app(None, vec![vault(&dir)]));
    let first = save_title(&client, &name, "Note").await;
    first.assert_json(json!({ "path": "Note.md" })).await;
    let second = save_title(&client, &name, "Note").await;
    second.assert_json(json!({ "path": "Note 1.md" })).await;
}

async fn save_title<E>(
    client: &TestClient<E>,
    vault_name: &str,
    title: &str,
) -> poem::test::TestResponse
where
    E: poem::Endpoint + 'static,
{
    client
        .post("/api/notes")
        .header("Content-Type", "application/json")
        .body(
            json!({"vault": vault_name, "name": title, "properties": {}, "content": ""})
                .to_string(),
        )
        .send()
        .await
}

#[tokio::test]
async fn save_note_rejects_unknown_vault_and_traversal_names() {
    let dir = TempVault::new("rejects");
    let name = vault(&dir).name;
    let client = TestClient::new(app(None, vec![vault(&dir)]));
    let unknown = client
        .post("/api/notes")
        .header("Content-Type", "application/json")
        .body(json!({"vault": "Nope", "name": "x", "properties": {}, "content": ""}).to_string())
        .send()
        .await;
    unknown.assert_status(StatusCode::NOT_FOUND);

    let traversal = client
        .post("/api/notes")
        .header("Content-Type", "application/json")
        .body(
            json!({"vault": name, "name": "../evil", "properties": {}, "content": ""}).to_string(),
        )
        .send()
        .await;
    traversal.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn preflight_allows_extension_origins_only() {
    let dir = TempVault::new("cors");
    let client = TestClient::new(app(None, vec![vault(&dir)]));
    let allowed = client
        .request(Method::OPTIONS, "/api/vaults")
        .header("Origin", "chrome-extension://abc123")
        .header("Access-Control-Request-Method", "GET")
        .send()
        .await;
    allowed.assert_status_is_ok();
    allowed.assert_header("Access-Control-Allow-Origin", "chrome-extension://abc123");

    let refused = client
        .request(Method::OPTIONS, "/api/vaults")
        .header("Origin", "https://evil.example")
        .header("Access-Control-Request-Method", "GET")
        .send()
        .await;
    refused.assert_status(StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn webpage_origins_are_refused_but_plain_clients_pass() {
    let dir = TempVault::new("origins");
    let client = TestClient::new(app(None, vec![vault(&dir)]));
    let webpage = client
        .get("/api/vaults")
        .header("Origin", "https://evil.example")
        .send()
        .await;
    webpage.assert_status(StatusCode::FORBIDDEN);

    let plain = client.get("/api/vaults").send().await;
    plain.assert_status_is_ok();
}

#[tokio::test]
async fn oversize_post_is_rejected() {
    let dir = TempVault::new("oversize");
    let client = TestClient::new(app(None, vec![vault(&dir)]));
    let response = client
        .post("/api/notes")
        .header("Content-Type", "application/json")
        .header("Content-Length", (MAX_BODY_BYTES + 2).to_string())
        .body("{}")
        .send()
        .await;
    response.assert_status(StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn non_json_save_body_is_rejected() {
    let dir = TempVault::new("content-type");
    let client = TestClient::new(app(None, vec![vault(&dir)]));
    let response = client
        .post("/api/notes")
        .header("Content-Type", "text/plain")
        .body(json!({"vault": "v", "name": "n", "properties": {}, "content": ""}).to_string())
        .send()
        .await;
    response.assert_status(StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn xml_media_types_are_rejected_at_the_boundary() {
    let dir = TempVault::new("xml-guard");
    let client = TestClient::new(app(None, vec![vault(&dir)]));
    for content_type in [
        "application/xml",
        "text/xml",
        "application/atom+xml",
        "application/xml; charset=utf-8",
    ] {
        let response = client
            .post("/api/notes")
            .header("Content-Type", content_type)
            .body("<note/>")
            .send()
            .await;
        response.assert_status(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    // Requests without a content type are untouched.
    let plain = client.get("/ping").send().await;
    plain.assert_status_is_ok();
}

#[tokio::test]
async fn openapi_spec_advertises_no_xml_media_types() {
    let dir = TempVault::new("no-xml-spec");
    let client = TestClient::new(app(None, vec![vault(&dir)]));
    let spec = client.get("/openapi.json").send().await;
    spec.assert_status_is_ok();
    let body: Value = spec
        .0
        .into_body()
        .into_json::<Value>()
        .await
        .unwrap_or_default();
    let mut xml = Vec::new();
    collect_xml_media_types(&body, &mut xml);
    assert!(
        xml.is_empty(),
        "the spec advertises XML media types; quick-xml would be reachable: {xml:?}"
    );
}

/// Collects every `content` map key (a media type) that mentions XML.
fn collect_xml_media_types(value: &Value, found: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key == "content"
                    && let Value::Object(content) = child
                {
                    for media_type in content.keys() {
                        if media_type.to_ascii_lowercase().contains("xml") {
                            found.push(media_type.clone());
                        }
                    }
                } else {
                    collect_xml_media_types(child, found);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_xml_media_types(item, found);
            }
        }
        _ => {}
    }
}

#[tokio::test]
async fn unknown_routes_and_methods_are_rejected() {
    let dir = TempVault::new("routing");
    let client = TestClient::new(app(None, vec![vault(&dir)]));
    let missing = client.get("/api/nope").send().await;
    missing.assert_status(StatusCode::NOT_FOUND);

    let wrong_method = client.get("/api/notes").send().await;
    wrong_method.assert_status(StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn openapi_spec_and_docs_are_served() {
    let dir = TempVault::new("spec");
    let client = TestClient::new(app(None, vec![vault(&dir)]));
    let spec = client.get("/openapi.json").send().await;
    spec.assert_status_is_ok();
    let body: Value = spec
        .0
        .into_body()
        .into_json::<Value>()
        .await
        .unwrap_or_default();
    assert!(body["paths"]["/ping"].is_object());
    assert!(body["paths"]["/api/vaults"].is_object());
    assert!(body["paths"]["/api/notes"].is_object());

    let docs = client.get("/docs").send().await;
    docs.assert_status_is_ok();
}

#[test]
fn manager_binds_ephemeral_port_and_stops() {
    let dir = TempVault::new("manager");
    let mut manager = ServerManager::new();
    manager
        .apply_with_context(
            Some(ServerConfig {
                port: 0,
                token: None,
            }),
            Some(context(None, vec![vault(&dir)])),
        )
        .unwrap();
    assert!(matches!(manager.status(), ServerStatus::Running(port) if port > 0));
    manager.stop();
    assert_eq!(manager.status(), ServerStatus::Stopped);
}

#[test]
fn manager_reports_bind_failure_without_losing_state() {
    let held = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = held.local_addr().unwrap().port();

    let dir = TempVault::new("bind");
    let mut manager = ServerManager::new();
    let result = manager.apply_with_context(
        Some(ServerConfig { port, token: None }),
        Some(context(None, vec![vault(&dir)])),
    );
    assert!(result.is_err());
    assert!(matches!(manager.status(), ServerStatus::Failed(_)));
}
