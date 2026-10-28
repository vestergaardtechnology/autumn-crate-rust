//! The [`Autumn`] client and its configuration.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::header::{
    HeaderMap, HeaderName, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE, RETRY_AFTER,
    USER_AGENT,
};
use reqwest::Url;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::{ApiError, Error, ResponseValidationError, Result};
use crate::generated_meta::{OPENAPI_DOC_VERSION, SOURCE_SDK_VERSION};

/// The Autumn API server used unless [`AutumnBuilder::server_url`] overrides it.
pub const DEFAULT_SERVER_URL: &str = "https://api.useautumn.com";

/// Operations that may be served by the edge (`edge_url`) when edge routing is on.
const EDGE_OPERATION_IDS: &[&str] = &["check", "finalizeLock"];
/// How long the edge gets to answer before the call falls back to the API.
const EDGE_TIMEOUT: Duration = Duration::from_secs(1);
/// Header set by the edge when it forwarded the call to the API itself.
const EDGE_FORWARDED_HEADER: &str = "x-atom-forwarded";

/// Operations that fail open (see [`AutumnBuilder::fail_open`]).
const FAIL_OPEN_OPERATION_IDS: &[&str] = &["check", "track"];
const FAIL_OPEN_LOG_MESSAGE: &str =
    "[Autumn] Request failed \u{2014} failing open. Learn more: https://docs.useautumn.com/documentation/fail-open";

/// Operations that get a default timeout when none is configured.
const AUTO_TIMEOUT_OPERATION_IDS: &[&str] = &["check", "track"];
const AUTO_TIMEOUT: Duration = Duration::from_secs(5);

const IDEMPOTENCY_KEY_HEADER: &str = "Idempotency-Key";
const API_VERSION_HEADER: &str = "x-api-version";

/// Status codes retried by default when a retry strategy is configured.
const DEFAULT_RETRY_CODES: &[&str] = &["429", "500", "502", "503", "504"];

type SecretKeyFuture = Pin<Box<dyn Future<Output = String> + Send>>;
type SecretKeyFn = dyn Fn() -> SecretKeyFuture + Send + Sync;

/// Static description of an API operation, generated from the JS SDK.
#[derive(Debug)]
pub(crate) struct Operation {
    pub id: &'static str,
    pub path: &'static str,
    pub method: &'static str,
    pub sends_body: bool,
    pub success_statuses: &'static [u16],
    pub retry_codes: &'static [&'static str],
}

/// How failed requests are retried.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum RetryConfig {
    /// Never retry (the default).
    #[default]
    None,
    /// Retry with exponential backoff.
    Backoff {
        /// Backoff timing.
        backoff: BackoffStrategy,
        /// Also retry when the request could not be sent or timed out.
        retry_connection_errors: bool,
    },
}

impl RetryConfig {
    /// Exponential backoff with the default timings.
    pub fn backoff() -> Self {
        RetryConfig::Backoff {
            backoff: BackoffStrategy::default(),
            retry_connection_errors: false,
        }
    }
}

/// Exponential backoff timings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BackoffStrategy {
    /// Base interval between attempts (default 500ms).
    pub initial_interval: Duration,
    /// Upper bound for a single wait (default 60s).
    pub max_interval: Duration,
    /// Growth exponent (default 1.5).
    pub exponent: f64,
    /// Stop retrying once this much time has passed (default 1h).
    pub max_elapsed_time: Duration,
}

impl Default for BackoffStrategy {
    fn default() -> Self {
        BackoffStrategy {
            initial_interval: Duration::from_millis(500),
            max_interval: Duration::from_secs(60),
            exponent: 1.5,
            max_elapsed_time: Duration::from_secs(3600),
        }
    }
}

/// Per-call overrides, applied with [`Autumn::with_options`].
#[derive(Debug, Clone, Default)]
pub struct RequestOptions {
    /// Overall timeout for the call, including retries.
    pub timeout: Option<Duration>,
    /// Retry strategy for the call.
    pub retries: Option<RetryConfig>,
    /// Status codes (e.g. `"429"`, `"5XX"`) to retry.
    pub retry_codes: Option<Vec<String>>,
    /// Server URL for the call.
    pub server_url: Option<String>,
    /// Extra headers; these override the SDK's own headers.
    pub headers: HeaderMap,
}

impl RequestOptions {
    /// Empty options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the overall timeout for the call.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Sets the retry strategy for the call.
    pub fn retries(mut self, retries: RetryConfig) -> Self {
        self.retries = Some(retries);
        self
    }

    /// Sets the status codes to retry, e.g. `["429", "5XX"]`.
    pub fn retry_codes<I, S>(mut self, codes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.retry_codes = Some(codes.into_iter().map(Into::into).collect());
        self
    }

    /// Sends the call to another server.
    pub fn server_url(mut self, url: impl Into<String>) -> Self {
        self.server_url = Some(url.into());
        self
    }

    /// Adds a header to the call.
    ///
    /// # Panics
    ///
    /// Panics if the name or value is not a valid HTTP header.
    pub fn header(mut self, name: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        let name = HeaderName::from_bytes(name.as_ref().as_bytes()).expect("invalid header name");
        let value = HeaderValue::from_str(value.as_ref()).expect("invalid header value");
        self.headers.insert(name, value);
        self
    }

    /// Sets the `Idempotency-Key` header for the call.
    pub fn idempotency_key(self, key: impl AsRef<str>) -> Self {
        self.header(IDEMPOTENCY_KEY_HEADER, key)
    }
}

/// Client for the Autumn API.
///
/// Cheap to clone; clones share the connection pool and configuration.
///
/// ```no_run
/// # async fn run() -> autumn_sdk::Result<()> {
/// use autumn_sdk::{Autumn, models::CheckParams};
///
/// let autumn = Autumn::new("am_sk_test_...");
/// let res = autumn.check(CheckParams::new("cus_123", "messages")).await?;
/// # Ok(()) }
/// ```
#[derive(Clone)]
pub struct Autumn {
    inner: Arc<Inner>,
    use_edge: bool,
    call: Option<Arc<RequestOptions>>,
}

struct Inner {
    http: reqwest::Client,
    base_url: Url,
    edge_url: Option<Url>,
    secret_key: Option<SecretKeySource>,
    x_api_version: Option<String>,
    fail_open: bool,
    user_agent: String,
    retry_config: RetryConfig,
    timeout: Option<Duration>,
    debug: bool,
}

#[derive(Clone)]
enum SecretKeySource {
    Static(String),
    Dynamic(Arc<SecretKeyFn>),
}

impl fmt::Debug for Autumn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Autumn")
            .field("server_url", &self.inner.base_url.as_str())
            .field("edge_url", &self.inner.edge_url.as_ref().map(Url::as_str))
            .field("use_edge", &self.use_edge)
            .field("has_secret_key", &self.inner.secret_key.is_some())
            .field("x_api_version", &self.inner.x_api_version)
            .field("fail_open", &self.inner.fail_open)
            .finish_non_exhaustive()
    }
}

impl Autumn {
    /// A client authenticated with `secret_key`; everything else is read from
    /// the environment or defaults (see [`AutumnBuilder`]).
    pub fn new(secret_key: impl Into<String>) -> Self {
        Self::builder()
            .secret_key(secret_key)
            .build()
            .expect("the default Autumn configuration is valid")
    }

    /// A client configured from the environment: `AUTUMN_SECRET_KEY`,
    /// `AUTUMN_X_API_VERSION`, `AUTUMN_FAIL_OPEN` and `AUTUMN_DEBUG`.
    pub fn from_env() -> Self {
        Self::builder()
            .build()
            .expect("the default Autumn configuration is valid")
    }

    /// A builder for a customized client.
    pub fn builder() -> AutumnBuilder {
        AutumnBuilder::default()
    }

    /// This client with edge routing on: each call the edge serves (`check`,
    /// `balances().finalize`) is asked of the edge at `edge_url` first, and of
    /// the Autumn API when the edge is down.
    pub fn edge(&self) -> Autumn {
        Autumn {
            use_edge: true,
            ..self.clone()
        }
    }

    /// This client with per-call overrides applied to every call made through it.
    ///
    /// ```no_run
    /// # async fn run(autumn: autumn_sdk::Autumn) -> autumn_sdk::Result<()> {
    /// use std::time::Duration;
    /// use autumn_sdk::{RequestOptions, RetryConfig};
    ///
    /// let customers = autumn
    ///     .with_options(RequestOptions::new().timeout(Duration::from_secs(10)).retries(RetryConfig::backoff()))
    ///     .customers()
    ///     .list(Default::default())
    ///     .await?;
    /// # Ok(()) }
    /// ```
    pub fn with_options(&self, options: RequestOptions) -> Autumn {
        Autumn {
            call: Some(Arc::new(options)),
            ..self.clone()
        }
    }

    /// The base URL of the Autumn API this client talks to.
    pub fn server_url(&self) -> &Url {
        &self.inner.base_url
    }

    pub(crate) async fn execute<Req, Res>(&self, op: &Operation, body: Option<&Req>) -> Result<Res>
    where
        Req: Serialize + ?Sized,
        Res: DeserializeOwned,
    {
        let body = match body {
            Some(b) if op.sends_body => Some(serde_json::to_vec(b).map_err(Error::Serialization)?),
            _ => None,
        };
        let opts = self.call.as_deref();
        let fail_open = self.inner.fail_open && FAIL_OPEN_OPERATION_IDS.contains(&op.id);

        let timeout = opts
            .and_then(|o| o.timeout)
            .or(self.inner.timeout)
            .or_else(|| {
                AUTO_TIMEOUT_OPERATION_IDS
                    .contains(&op.id)
                    .then_some(AUTO_TIMEOUT)
            });

        let target = self.target(op, opts)?;
        let headers = self.headers(op, opts, body.as_deref()).await?;
        let attempt = self.send_with_retries(op, opts, &target, &headers, body, fail_open);
        let outcome = match timeout {
            Some(t) => tokio::time::timeout(t, attempt)
                .await
                .unwrap_or(Err(Error::Timeout)),
            None => attempt.await,
        };

        let raw = match outcome {
            Ok(raw) if fail_open && raw.status >= 500 => {
                return fail_open_response(op, &raw.status.to_string())
            }
            Err(e @ (Error::Timeout | Error::Connection(_))) if fail_open => {
                return fail_open_response(op, &e.to_string())
            }
            other => other?,
        };

        if op.success_statuses.contains(&raw.status) {
            serde_json::from_slice(&raw.body).map_err(|source| {
                Error::ResponseValidation(Box::new(ResponseValidationError {
                    status: raw.status,
                    body: String::from_utf8_lossy(&raw.body).into_owned(),
                    headers: raw.headers,
                    source,
                }))
            })
        } else {
            let summary = if (400..600).contains(&raw.status) {
                "API error occurred"
            } else {
                "Unexpected Status or Content-Type"
            };
            Err(Error::Api(Box::new(ApiError::new(
                summary,
                raw.status,
                raw.headers,
                String::from_utf8_lossy(&raw.body).into_owned(),
            ))))
        }
    }

    fn target(&self, op: &Operation, opts: Option<&RequestOptions>) -> Result<Target> {
        let base = match opts.and_then(|o| o.server_url.as_deref()) {
            Some(url) => parse_base_url(url)?,
            None => self.inner.base_url.clone(),
        };
        let url = base
            .join(op.path)
            .map_err(|e| Error::InvalidRequest(format!("invalid URL for {}: {e}", op.path)))?;
        let edge_url = match &self.inner.edge_url {
            Some(edge)
                if self.use_edge
                    && EDGE_OPERATION_IDS.contains(&op.id)
                    && base == self.inner.base_url =>
            {
                Some(
                    edge.join(op.path)
                        .map_err(|e| Error::InvalidRequest(format!("invalid edge URL: {e}")))?,
                )
            }
            _ => None,
        };
        let method = reqwest::Method::from_bytes(op.method.as_bytes())
            .map_err(|e| Error::InvalidRequest(format!("invalid method {}: {e}", op.method)))?;
        Ok(Target {
            method,
            url,
            edge_url,
        })
    }

    async fn headers(
        &self,
        op: &Operation,
        opts: Option<&RequestOptions>,
        body: Option<&[u8]>,
    ) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        if body.is_some() {
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        }
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        if let Some(version) = &self.inner.x_api_version {
            headers.insert(API_VERSION_HEADER, header_value(version)?);
        }
        let key = match &self.inner.secret_key {
            Some(SecretKeySource::Static(k)) => Some(k.clone()),
            Some(SecretKeySource::Dynamic(f)) => Some(f().await),
            None => None,
        };
        if let Some(key) = key.filter(|k| !k.is_empty()) {
            let mut v = header_value(&format!("Bearer {key}"))?;
            v.set_sensitive(true);
            headers.insert(AUTHORIZATION, v);
        }
        if let Some(opts) = opts {
            for (k, v) in &opts.headers {
                headers.insert(k.clone(), v.clone());
            }
        }
        headers.insert(USER_AGENT, header_value(&self.inner.user_agent)?);
        if self.use_edge && is_write(op, body) && !headers.contains_key(IDEMPOTENCY_KEY_HEADER) {
            headers.insert(
                IDEMPOTENCY_KEY_HEADER,
                header_value(&uuid::Uuid::new_v4().to_string())?,
            );
        }
        Ok(headers)
    }

    async fn send_with_retries(
        &self,
        op: &Operation,
        opts: Option<&RequestOptions>,
        target: &Target,
        headers: &HeaderMap,
        body: Option<Vec<u8>>,
        fail_open: bool,
    ) -> Result<RawResponse> {
        let config = opts
            .and_then(|o| o.retries.clone())
            .unwrap_or_else(|| self.inner.retry_config.clone());
        let codes: Vec<&str> = match opts.and_then(|o| o.retry_codes.as_ref()) {
            Some(codes) => codes.iter().map(String::as_str).collect(),
            None if !op.retry_codes.is_empty() => op.retry_codes.to_vec(),
            None => DEFAULT_RETRY_CODES.to_vec(),
        };
        let RetryConfig::Backoff {
            backoff,
            retry_connection_errors,
        } = config
        else {
            return self.send_once(target, headers, body.as_deref()).await;
        };

        let start = Instant::now();
        let mut attempt: i32 = 0;
        loop {
            let res = self.send_once(target, headers, body.as_deref()).await;
            let retry_after = match &res {
                // A server error on a fail-open call is answered, not retried.
                Ok(r) if fail_open && r.status >= 500 => return res,
                Ok(r) if status_matches(r.status, &codes) => Some(retry_after(&r.headers)),
                Ok(_) => return res,
                Err(Error::Timeout | Error::Connection(_))
                    if retry_connection_errors && !fail_open =>
                {
                    None
                }
                Err(_) => return res,
            };
            if start.elapsed() > backoff.max_elapsed_time {
                return res;
            }
            let wait = match retry_after.flatten() {
                Some(d) if !d.is_zero() => d,
                _ => {
                    let jitter = rand::random::<f64>() * 1000.0;
                    let ms = backoff.initial_interval.as_millis() as f64
                        * f64::from(attempt).powf(backoff.exponent)
                        + jitter;
                    Duration::from_millis(ms as u64)
                }
            };
            tokio::time::sleep(wait.min(backoff.max_interval)).await;
            attempt += 1;
        }
    }

    async fn send_once(
        &self,
        target: &Target,
        headers: &HeaderMap,
        body: Option<&[u8]>,
    ) -> Result<RawResponse> {
        if let Some(edge_url) = &target.edge_url {
            match self
                .send_http(&target.method, edge_url, headers, body, Some(EDGE_TIMEOUT))
                .await
            {
                Ok(r) if !(r.status >= 500 && !r.headers.contains_key(EDGE_FORWARDED_HEADER)) => {
                    return Ok(r)
                }
                Ok(r) => {
                    log::debug!(target: "autumn_sdk", "edge returned {}, falling back to the API", r.status)
                }
                Err(e) => {
                    log::debug!(target: "autumn_sdk", "edge unreachable ({e}), falling back to the API")
                }
            }
        }
        self.send_http(&target.method, &target.url, headers, body, None)
            .await
    }

    async fn send_http(
        &self,
        method: &reqwest::Method,
        url: &Url,
        headers: &HeaderMap,
        body: Option<&[u8]>,
        timeout: Option<Duration>,
    ) -> Result<RawResponse> {
        let mut req = self
            .inner
            .http
            .request(method.clone(), url.clone())
            .headers(headers.clone());
        if let Some(body) = body {
            req = req.body(body.to_vec());
        }
        if let Some(t) = timeout {
            req = req.timeout(t);
        }
        if self.inner.debug {
            log_request(method, url, headers, body);
        }
        let map_err = |e: reqwest::Error| {
            if e.is_timeout() {
                Error::Timeout
            } else {
                Error::Connection(e)
            }
        };
        let resp = req.send().await.map_err(map_err)?;
        let status = resp.status().as_u16();
        let headers = resp.headers().clone();
        let body = resp.bytes().await.map_err(map_err)?.to_vec();
        if self.inner.debug {
            log_response(method, url, status, &headers, &body);
        }
        Ok(RawResponse {
            status,
            headers,
            body,
        })
    }
}

struct Target {
    method: reqwest::Method,
    url: Url,
    edge_url: Option<Url>,
}

struct RawResponse {
    status: u16,
    headers: HeaderMap,
    body: Vec<u8>,
}

/// Builder for [`Autumn`].
///
/// Unset options fall back to the same environment variables the JS SDK
/// reads: `AUTUMN_SECRET_KEY`, `AUTUMN_X_API_VERSION`, `AUTUMN_FAIL_OPEN`
/// and `AUTUMN_DEBUG`.
#[derive(Default)]
pub struct AutumnBuilder {
    secret_key: Option<SecretKeySource>,
    x_api_version: Option<String>,
    fail_open: Option<bool>,
    server_url: Option<String>,
    edge_url: Option<String>,
    use_edge: bool,
    user_agent: Option<String>,
    retry_config: RetryConfig,
    timeout: Option<Duration>,
    debug: Option<bool>,
    http_client: Option<reqwest::Client>,
}

impl fmt::Debug for AutumnBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AutumnBuilder")
            .field("has_secret_key", &self.secret_key.is_some())
            .field("x_api_version", &self.x_api_version)
            .field("fail_open", &self.fail_open)
            .field("server_url", &self.server_url)
            .field("edge_url", &self.edge_url)
            .field("use_edge", &self.use_edge)
            .finish_non_exhaustive()
    }
}

impl AutumnBuilder {
    /// The secret key (`am_sk_...`). Defaults to `AUTUMN_SECRET_KEY`.
    pub fn secret_key(mut self, key: impl Into<String>) -> Self {
        self.secret_key = Some(SecretKeySource::Static(key.into()));
        self
    }

    /// Resolves the secret key before every request, e.g. from a secret store.
    pub fn secret_key_provider<F, Fut>(mut self, provider: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = String> + Send + 'static,
    {
        self.secret_key = Some(SecretKeySource::Dynamic(Arc::new(move || {
            Box::pin(provider())
        })));
        self
    }

    /// The `x-api-version` header sent with every call. Defaults to
    /// `AUTUMN_X_API_VERSION`, else the API version the SDK was generated for.
    pub fn x_api_version(mut self, version: impl Into<String>) -> Self {
        self.x_api_version = Some(version.into());
        self
    }

    /// Fail open (default `true`, or `AUTUMN_FAIL_OPEN`): when `check` or
    /// `track` cannot reach Autumn (network error, timeout or 5XX), return an
    /// "allowed" response instead of an error so outages never block your
    /// users. See <https://docs.useautumn.com/documentation/fail-open>.
    pub fn fail_open(mut self, fail_open: bool) -> Self {
        self.fail_open = Some(fail_open);
        self
    }

    /// Overrides the API server (default `https://api.useautumn.com`).
    pub fn server_url(mut self, url: impl Into<String>) -> Self {
        self.server_url = Some(url.into());
        self
    }

    /// The edge to ask first for the calls it serves; without one, every call
    /// goes to the Autumn API.
    pub fn edge_url(mut self, url: impl Into<String>) -> Self {
        self.edge_url = Some(url.into());
        self
    }

    /// Send every call the edge serves (e.g. `check`) to `edge_url` first,
    /// falling back to the Autumn API when the edge is down. See also
    /// [`Autumn::edge`].
    pub fn use_edge(mut self, use_edge: bool) -> Self {
        self.use_edge = use_edge;
        self
    }

    /// Overrides the `User-Agent` header.
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Default retry strategy (default: no retries).
    pub fn retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    /// Default overall timeout per call. Without one, `check` and `track` time
    /// out after 5 seconds and other calls never do.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Log every request and response through the [`log`] crate at debug
    /// level, with the `Authorization` header redacted. Defaults to
    /// `AUTUMN_DEBUG`.
    pub fn debug_logging(mut self, debug: bool) -> Self {
        self.debug = Some(debug);
        self
    }

    /// Uses your own `reqwest` client (proxies, TLS settings, connection pool).
    pub fn http_client(mut self, client: reqwest::Client) -> Self {
        self.http_client = Some(client);
        self
    }

    /// Builds the client.
    pub fn build(self) -> Result<Autumn> {
        let base_url = parse_base_url(self.server_url.as_deref().unwrap_or(DEFAULT_SERVER_URL))?;
        let edge_url = self.edge_url.as_deref().map(parse_base_url).transpose()?;
        let secret_key = self
            .secret_key
            .or_else(|| env("AUTUMN_SECRET_KEY").map(SecretKeySource::Static));
        let x_api_version = self
            .x_api_version
            .or_else(|| env("AUTUMN_X_API_VERSION"))
            .or_else(|| Some(OPENAPI_DOC_VERSION.to_owned()).filter(|v| !v.is_empty()));
        let fail_open = self.fail_open.unwrap_or_else(|| {
            env("AUTUMN_FAIL_OPEN").map_or(true, |v| !v.eq_ignore_ascii_case("false"))
        });
        let debug = self.debug.unwrap_or_else(|| {
            env("AUTUMN_DEBUG")
                .is_some_and(|v| !matches!(v.to_ascii_lowercase().as_str(), "" | "0" | "false"))
        });
        let http = match self.http_client {
            Some(c) => c,
            None => reqwest::Client::builder()
                .build()
                .map_err(|e| Error::InvalidRequest(e.to_string()))?,
        };
        Ok(Autumn {
            inner: Arc::new(Inner {
                http,
                base_url,
                edge_url,
                secret_key,
                x_api_version,
                fail_open,
                user_agent: self.user_agent.unwrap_or_else(default_user_agent),
                retry_config: self.retry_config,
                timeout: self.timeout,
                debug,
            }),
            use_edge: self.use_edge,
            call: None,
        })
    }
}

fn default_user_agent() -> String {
    format!(
        "autumn-sdk-rust/{} (autumn-js {SOURCE_SDK_VERSION}; api {OPENAPI_DOC_VERSION})",
        env!("CARGO_PKG_VERSION")
    )
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

fn parse_base_url(url: &str) -> Result<Url> {
    let mut u = Url::parse(url)
        .map_err(|e| Error::InvalidRequest(format!("invalid server URL {url:?}: {e}")))?;
    if u.cannot_be_a_base() {
        return Err(Error::InvalidRequest(format!("invalid server URL {url:?}")));
    }
    let path = format!("{}/", u.path().trim_end_matches('/'));
    u.set_path(&path);
    u.set_fragment(None);
    Ok(u)
}

fn header_value(v: &str) -> Result<HeaderValue> {
    HeaderValue::from_str(v)
        .map_err(|e| Error::InvalidRequest(format!("invalid header value: {e}")))
}

/// Whether a call changes state, so retries through the edge must be idempotent.
fn is_write(op: &Operation, body: Option<&[u8]>) -> bool {
    if op.id == "finalizeLock" {
        return true;
    }
    if op.id != "check" {
        return false;
    }
    let Some(sent) = body.and_then(|b| serde_json::from_slice::<serde_json::Value>(b).ok()) else {
        return false;
    };
    sent.get("send_event") == Some(&serde_json::Value::Bool(true)) || sent.get("lock").is_some()
}

fn status_matches(status: u16, codes: &[&str]) -> bool {
    let actual = status.to_string();
    codes.iter().any(|code| {
        let code = code.trim();
        if code.len() == 3 && code[1..].eq_ignore_ascii_case("xx") {
            actual.starts_with(&code[..1])
        } else {
            code == actual
        }
    })
}

fn retry_after(headers: &HeaderMap) -> Option<Duration> {
    let v = headers.get(RETRY_AFTER)?.to_str().ok()?.trim();
    if let Ok(secs) = v.parse::<u64>() {
        return Some(Duration::from_secs(secs));
    }
    let when = httpdate::parse_http_date(v).ok()?;
    when.duration_since(std::time::SystemTime::now()).ok()
}

fn fail_open_response<Res: DeserializeOwned>(op: &Operation, cause: &str) -> Result<Res> {
    let body = match op.id {
        "check" => {
            serde_json::json!({ "allowed": true, "customer_id": null, "balance": null, "flag": null })
        }
        _ => serde_json::json!({ "customer_id": null, "value": 0, "balance": null }),
    };
    log::error!(target: "autumn_sdk", "{FAIL_OPEN_LOG_MESSAGE}");
    log::error!(target: "autumn_sdk", "  Operation: {} | Error: {cause}", op.id);
    serde_json::from_value(body.clone()).map_err(|source| {
        Error::ResponseValidation(Box::new(ResponseValidationError {
            status: 200,
            body: body.to_string(),
            headers: HeaderMap::new(),
            source,
        }))
    })
}

fn log_request(method: &reqwest::Method, url: &Url, headers: &HeaderMap, body: Option<&[u8]>) {
    log::debug!(target: "autumn_sdk", "> Request: {method} {url}");
    log_headers(headers);
    if let Some(body) = body {
        log::debug!(target: "autumn_sdk", "Body: {}", String::from_utf8_lossy(body));
    }
}

fn log_response(
    method: &reqwest::Method,
    url: &Url,
    status: u16,
    headers: &HeaderMap,
    body: &[u8],
) {
    log::debug!(target: "autumn_sdk", "< Response: {method} {url}");
    log::debug!(target: "autumn_sdk", "Status Code: {status}");
    log_headers(headers);
    log::debug!(target: "autumn_sdk", "Body: {}", String::from_utf8_lossy(body));
}

fn log_headers(headers: &HeaderMap) {
    for (k, v) in headers {
        let shown = if k == AUTHORIZATION {
            "[REDACTED]"
        } else {
            v.to_str().unwrap_or("<binary>")
        };
        log::debug!(target: "autumn_sdk", "{k}: {shown}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_code_patterns() {
        assert!(status_matches(503, &["5XX"]));
        assert!(status_matches(429, &["429", "500"]));
        assert!(!status_matches(404, &["429", "5xx"]));
    }

    #[test]
    fn base_url_gets_trailing_slash() {
        let u = parse_base_url("https://example.com/api").unwrap();
        assert_eq!(
            u.join("v1/balances.check").unwrap().as_str(),
            "https://example.com/api/v1/balances.check"
        );
        let u = parse_base_url("https://api.useautumn.com").unwrap();
        assert_eq!(
            u.join("v1/balances.check").unwrap().as_str(),
            "https://api.useautumn.com/v1/balances.check"
        );
    }

    #[test]
    fn write_detection() {
        let op = Operation {
            id: "check",
            path: "v1/balances.check",
            method: "POST",
            sends_body: true,
            success_statuses: &[200],
            retry_codes: &[],
        };
        assert!(is_write(&op, Some(br#"{"send_event":true}"#)));
        assert!(is_write(&op, Some(br#"{"lock":{"lock_id":"x"}}"#)));
        assert!(!is_write(&op, Some(br#"{"send_event":false}"#)));
    }
}
