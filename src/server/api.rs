//! The `OpenAPI` surface of the local server:
//! typed handlers, DTOs, the body-size guard, and the JSON-only media guard.

use std::collections::BTreeMap;
use std::sync::Arc;

use poem::error::ReadBodyError;
use poem::{Body, Endpoint, IntoResponse, Request, Response, Result, http::Method, http::header};
use poem_openapi::{Object, OpenApi, payload::Json};

use super::error::ApiError;
use super::vaults::{self, Vault};

/// Largest accepted request body, so giant clips cannot exhaust memory.
pub const MAX_BODY_BYTES: usize = 10 * 1024 * 1024;

/// Shared dependencies of the API handlers, injectable for tests.
pub struct ApiContext {
    /// Optional bearer token; `None` or empty disables authentication.
    pub token: Option<String>,
    /// The vaults clients may save into, most recently used first.
    pub vaults: Arc<dyn Fn() -> Vec<Vault> + Send + Sync>,
}

impl ApiContext {
    pub fn new(token: Option<String>, vaults: Arc<dyn Fn() -> Vec<Vault> + Send + Sync>) -> Self {
        Self { token, vaults }
    }
}

#[derive(Object)]
struct PingResponse {
    /// Always `datalith`.
    app: String,
    /// The app's package version.
    version: String,
}

#[derive(Object)]
struct VaultDto {
    /// Display name of the vault.
    name: String,
    /// Absolute path of the vault on disk.
    path: String,
}

#[derive(Object)]
struct SaveNoteRequest {
    /// Vault by name or absolute path, as returned by `GET /api/vaults`.
    vault: String,
    /// Note name without the `.md` extension.
    name: String,
    /// Optional folder inside the vault, created on first save.
    folder: Option<String>,
    /// Frontmatter properties; every value is a string.
    properties: BTreeMap<String, String>,
    /// Markdown body below the frontmatter.
    content: String,
}

#[derive(Object)]
struct SaveNoteResponse {
    /// Path of the saved note, relative to the vault root.
    path: String,
}

pub struct Api {
    context: ApiContext,
}

#[OpenApi]
#[allow(clippy::unused_async)]
impl Api {
    /// Health check advertised to local clients.
    #[oai(path = "/ping", method = "get")]
    async fn ping(&self, req: &Request) -> Result<Json<PingResponse>, ApiError> {
        self.guard(req)?;
        Ok(Json(PingResponse {
            app: "datalith".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
        }))
    }

    /// The vaults clients may save into, most recently used first.
    #[oai(path = "/api/vaults", method = "get")]
    async fn list_vaults(&self, req: &Request) -> Result<Json<Vec<VaultDto>>, ApiError> {
        self.guard(req)?;
        let vaults = (self.context.vaults)();
        let entries = vaults
            .iter()
            .map(|vault| VaultDto {
                name: vault.name.clone(),
                path: vault.path.to_string_lossy().into_owned(),
            })
            .collect();
        Ok(Json(entries))
    }

    /// Save a Markdown note with YAML frontmatter into a vault.
    #[oai(path = "/api/notes", method = "post")]
    async fn save_note(
        &self,
        req: &Request,
        body: Json<SaveNoteRequest>,
    ) -> Result<Json<SaveNoteResponse>, ApiError> {
        self.guard(req)?;
        let path = vaults::write_note(
            &(self.context.vaults)(),
            &body.vault,
            &body.name,
            body.folder.as_deref(),
            &body.properties,
            &body.content,
        )?;
        Ok(Json(SaveNoteResponse { path }))
    }
}

impl Api {
    pub const fn new(context: ApiContext) -> Self {
        Self { context }
    }

    fn guard(&self, req: &Request) -> Result<(), ApiError> {
        match self.context.token.as_deref() {
            None | Some("") => Ok(()),
            Some(token) if authorized(req, token) => Ok(()),
            Some(_) => Err(ApiError::unauthorized()),
        }
    }
}

/// Constant-time equality so token comparison does not leak its prefix.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = u32::try_from(a.len() ^ b.len()).unwrap_or(u32::MAX);
    let max = a.len().max(b.len());
    for index in 0..max {
        let left = a.get(index).copied().unwrap_or(0);
        let right = b.get(index).copied().unwrap_or(0);
        diff |= u32::from(left ^ right);
    }
    diff == 0
}

fn authorized(req: &Request, token: &str) -> bool {
    let expected = format!("Bearer {token}");
    req.headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| constant_time_eq(value.as_bytes(), expected.as_bytes()))
}

/// Rejects oversized POST bodies before the JSON extractor buffers them:
/// a fast `Content-Length` check up front, then a hard cap on the actual
/// bytes so chunked requests (which carry no `Content-Length`) are covered.
/// Browser clients are already gated by the strict preflight; this guards
/// against accidental memory exhaustion by local clients.
pub struct BodyLimit {
    pub max_size: usize,
}

impl<E: Endpoint> poem::Middleware<E> for BodyLimit {
    type Output = BodyLimitEndpoint<E>;

    fn transform(&self, ep: E) -> Self::Output {
        BodyLimitEndpoint {
            ep,
            max_size: self.max_size,
        }
    }
}

pub struct BodyLimitEndpoint<E> {
    ep: E,
    max_size: usize,
}

impl<E: Endpoint> Endpoint for BodyLimitEndpoint<E> {
    type Output = Response;

    async fn call(&self, req: Request) -> Result<Self::Output> {
        if req.method() == Method::POST {
            let too_big = req
                .headers()
                .get(header::CONTENT_LENGTH)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<usize>().ok())
                .is_some_and(|length| length > self.max_size);
            if too_big {
                return Err(ApiError::payload_too_large().into());
            }
            let (parts, body) = req.into_parts();
            let bytes = match body.into_bytes_limit(self.max_size).await {
                Ok(bytes) => bytes,
                Err(ReadBodyError::PayloadTooLarge) => {
                    return Err(ApiError::payload_too_large().into());
                }
                Err(error) => {
                    return Err(ApiError::internal(format!(
                        "Failed to read request body: {error}"
                    ))
                    .into());
                }
            };
            return Ok(self
                .ep
                .call(Request::from_parts(parts, Body::from(bytes)))
                .await?
                .into_response());
        }
        Ok(self.ep.call(req).await?.into_response())
    }
}

/// Rejects every XML media type before any extractor can run.
/// The Local Server is JSON-only: poem-openapi links `quick-xml` for XML payload support,
/// and this boundary guarantees that code is never reached
/// (the advisories ignore in `deny.toml` depends on it).
pub struct RejectXml;

impl<E: Endpoint> poem::Middleware<E> for RejectXml {
    type Output = RejectXmlEndpoint<E>;

    fn transform(&self, ep: E) -> Self::Output {
        RejectXmlEndpoint { ep }
    }
}

pub struct RejectXmlEndpoint<E> {
    ep: E,
}

impl<E: Endpoint> Endpoint for RejectXmlEndpoint<E> {
    type Output = Response;

    async fn call(&self, req: Request) -> Result<Self::Output> {
        if req.content_type().is_some_and(is_xml_media_type) {
            return Err(ApiError::unsupported_media_type().into());
        }
        Ok(self.ep.call(req).await?.into_response())
    }
}

/// Matches `*/xml` and any `*/*+xml` media type, case-insensitively,
/// ignoring parameters (`application/xml; charset=utf-8` is XML).
fn is_xml_media_type(content_type: &str) -> bool {
    let Some((_, subtype)) = content_type
        .split(';')
        .next()
        .and_then(|mime| mime.trim().split_once('/'))
    else {
        return false;
    };
    let subtype = subtype.trim().to_ascii_lowercase();
    subtype == "xml" || subtype.ends_with("+xml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_compare_is_exact() {
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"secreT"));
        assert!(!constant_time_eq(b"secret", b"secre"));
        assert!(!constant_time_eq(b"", b"x"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn xml_media_types_are_detected() {
        for xml in [
            "application/xml",
            "text/xml",
            "APPLICATION/XML",
            "application/atom+xml",
            "application/rss+xml",
            "image/svg+xml",
            "application/xml; charset=utf-8",
            " text/xml ",
        ] {
            assert!(is_xml_media_type(xml), "should match: {xml}");
        }
    }

    #[test]
    fn non_xml_media_types_pass() {
        for other in [
            "application/json",
            "application/json; charset=utf-8",
            "text/plain",
            "text/xmli",
            "xml",
            "",
            "multipart/form-data; boundary=x",
        ] {
            assert!(!is_xml_media_type(other), "should not match: {other}");
        }
    }

    #[tokio::test]
    async fn oversized_body_without_content_length_is_rejected() {
        use poem::Middleware as _;
        use poem::http::StatusCode;

        let limited = BodyLimit { max_size: 8 }.transform(poem::endpoint::make_sync(|_| "ok"));
        let request = Request::builder().method(Method::POST).body("123456789");
        let error = limited.call(request).await.unwrap_err();
        assert_eq!(error.status(), StatusCode::PAYLOAD_TOO_LARGE);

        let within = BodyLimit { max_size: 8 }.transform(poem::endpoint::make_sync(|_| "ok"));
        let request = Request::builder().method(Method::POST).body("1234");
        let response = within.call(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
