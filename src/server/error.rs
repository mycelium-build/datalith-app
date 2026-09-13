//! HTTP error responses for the local server API.

use std::fmt;

use poem::error::ResponseError;
use poem::{IntoResponse, Response, http::StatusCode};
use poem_openapi::registry::{MetaResponse, MetaResponses};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    pub status: u16,
    pub message: String,
}

const ERROR_STATUSES: [(u16, &str); 8] = [
    (400, "Invalid request"),
    (401, "Missing or invalid bearer token"),
    (403, "Forbidden"),
    (404, "Not found"),
    (405, "Method not allowed"),
    (413, "Request body too large"),
    (415, "Only JSON requests are accepted"),
    (500, "Internal server error"),
];

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(400, message)
    }

    pub fn unauthorized() -> Self {
        Self::new(401, "Missing or invalid bearer token")
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(404, message)
    }

    pub fn payload_too_large() -> Self {
        Self::new(413, "Request body too large")
    }

    pub fn unsupported_media_type() -> Self {
        Self::new(415, "Only JSON requests are accepted")
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(500, message)
    }

    fn new(status: u16, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    pub fn body(&self) -> String {
        serde_json::json!({ "error": self.message }).to_string()
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} {}", self.status, self.message)
    }
}

impl std::error::Error for ApiError {}

impl ResponseError for ApiError {
    fn status(&self) -> StatusCode {
        StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    }

    fn as_response(&self) -> Response {
        self.clone().into_response()
    }
}

impl poem_openapi::ApiResponse for ApiError {
    fn meta() -> MetaResponses {
        MetaResponses {
            responses: ERROR_STATUSES
                .iter()
                .map(|(status, description)| MetaResponse {
                    description,
                    status: Some(*status),
                    status_range: None,
                    content: vec![],
                    headers: vec![],
                })
                .collect(),
        }
    }

    fn register(_registry: &mut poem_openapi::registry::Registry) {}

    fn from_parse_request_error(err: poem::Error) -> Self {
        Self::new(err.status().as_u16(), err.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        Response::builder()
            .status(status)
            .content_type("application/json")
            .body(self.body())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_is_json_object_with_error_key() {
        let error = ApiError::bad_request("nope");
        assert_eq!(error.body(), r#"{"error":"nope"}"#,);
    }

    #[test]
    fn display_is_status_plus_message() {
        assert_eq!(ApiError::not_found("gone").to_string(), "404 gone");
    }
}
