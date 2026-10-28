//! Error types.

use std::fmt;

use reqwest::header::HeaderMap;

/// Result alias used throughout the SDK.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Everything that can go wrong when calling the Autumn API.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The API answered with an error status (4XX/5XX) or an unexpected one.
    Api(Box<ApiError>),
    /// The API answered successfully but the body did not match the expected shape.
    ResponseValidation(Box<ResponseValidationError>),
    /// The request did not complete within the configured timeout.
    Timeout,
    /// The request could not be sent or the response could not be read.
    Connection(reqwest::Error),
    /// The request body could not be serialized.
    Serialization(serde_json::Error),
    /// The request could not be built (for example an invalid server URL).
    InvalidRequest(String),
}

impl Error {
    /// The HTTP status code, when the error came from an HTTP response.
    pub fn status(&self) -> Option<u16> {
        match self {
            Error::Api(e) => Some(e.status),
            Error::ResponseValidation(e) => Some(e.status),
            _ => None,
        }
    }

    /// The Autumn error code (e.g. `customer_not_found`), when the API returned one.
    pub fn code(&self) -> Option<&str> {
        match self {
            Error::Api(e) => e.code.as_deref(),
            _ => None,
        }
    }

    /// The [`ApiError`], if this is an error response from the API.
    pub fn as_api_error(&self) -> Option<&ApiError> {
        match self {
            Error::Api(e) => Some(e),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Api(e) => e.fmt(f),
            Error::ResponseValidation(e) => e.fmt(f),
            Error::Timeout => f.write_str("Request timed out"),
            Error::Connection(e) => write!(f, "Unable to make request: {e}"),
            Error::Serialization(e) => write!(f, "Input validation failed: {e}"),
            Error::InvalidRequest(msg) => write!(f, "Invalid request: {msg}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::ResponseValidation(e) => Some(&e.source),
            Error::Connection(e) => Some(e),
            Error::Serialization(e) => Some(e),
            _ => None,
        }
    }
}

impl From<ApiError> for Error {
    fn from(e: ApiError) -> Self {
        Error::Api(Box::new(e))
    }
}

/// An error response from the Autumn API.
#[derive(Debug, Clone)]
pub struct ApiError {
    /// HTTP status code.
    pub status: u16,
    /// Raw response body.
    pub body: String,
    /// Response headers.
    pub headers: HeaderMap,
    /// Response `content-type`, or an empty string.
    pub content_type: String,
    /// The Autumn error code from the body (e.g. `customer_not_found`), if any.
    pub code: Option<String>,
    /// The human readable message from the body, if any.
    pub message: Option<String>,
    summary: &'static str,
}

impl ApiError {
    pub(crate) fn new(
        summary: &'static str,
        status: u16,
        headers: HeaderMap,
        body: String,
    ) -> Self {
        let content_type = headers
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        let parsed: Option<serde_json::Value> = serde_json::from_str(&body).ok();
        let field = |name: &str| {
            parsed
                .as_ref()
                .and_then(|v| v.get(name))
                .and_then(|v| v.as_str())
                .map(str::to_owned)
        };
        ApiError {
            status,
            code: field("code"),
            message: field("message"),
            body,
            headers,
            content_type,
            summary,
        }
    }

    /// Deserializes the body as JSON.
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> serde_json::Result<T> {
        serde_json::from_str(&self.body)
    }
}

impl fmt::Display for ApiError {
    // Matches the message format of the JS SDK's `AutumnDefaultError`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: Status {}", self.summary, self.status)?;
        let ct = if self.content_type.is_empty() {
            "\"\""
        } else {
            self.content_type.as_str()
        };
        if ct != "application/json" {
            if ct.contains(' ') {
                write!(f, " Content-Type \"{ct}\"")?;
            } else {
                write!(f, " Content-Type {ct}")?;
            }
        }
        let body = if self.body.is_empty() {
            "\"\""
        } else {
            self.body.as_str()
        };
        f.write_str(if body.len() > 100 { "\n" } else { ". " })?;
        const MAX: usize = 10_000;
        if body.len() > MAX {
            let mut cut = MAX;
            while !body.is_char_boundary(cut) {
                cut -= 1;
            }
            write!(
                f,
                "Body: {}...and {} more chars",
                &body[..cut],
                body.len() - cut
            )
        } else {
            write!(f, "Body: {}", body.trim_end())
        }
    }
}

impl std::error::Error for ApiError {}

/// The API answered successfully but the body could not be parsed into the
/// expected type.
#[derive(Debug)]
pub struct ResponseValidationError {
    /// HTTP status code.
    pub status: u16,
    /// Raw response body.
    pub body: String,
    /// Response headers.
    pub headers: HeaderMap,
    /// The underlying deserialization error.
    pub source: serde_json::Error,
}

impl fmt::Display for ResponseValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Response validation failed: {}", self.source)
    }
}

impl std::error::Error for ResponseValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}
