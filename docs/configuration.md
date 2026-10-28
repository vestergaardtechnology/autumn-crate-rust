# Configuration

## Builder

```rust
use std::time::Duration;
use autumn_sdk::{Autumn, RetryConfig};

let autumn = Autumn::builder()
    .secret_key("am_sk_live_...")
    .retry_config(RetryConfig::backoff())
    .timeout(Duration::from_secs(10))
    .build()?;
```

| Builder method | Default | Notes |
|---|---|---|
| `secret_key(key)` | `AUTUMN_SECRET_KEY` | Sent as `Authorization: Bearer <key>`. |
| `secret_key_provider(\|\| async { .. })` | | Resolves the key before every request, e.g. from a secret manager. |
| `server_url(url)` | `https://api.useautumn.com` | Point at a self-hosted or mock server. |
| `x_api_version(v)` | `AUTUMN_X_API_VERSION`, else `2.6.0` | The API version the models were generated for. Override only if you know you need to. |
| `fail_open(bool)` | `AUTUMN_FAIL_OPEN`, else `true` | See [fail-open](fail-open-and-edge.md). |
| `timeout(d)` | none (5s for `check`/`track`) | Overall budget per call, including retries. |
| `retry_config(c)` | `RetryConfig::None` | See below. |
| `edge_url(url)` / `use_edge(bool)` | none / `false` | See [edge routing](fail-open-and-edge.md#edge-routing). |
| `user_agent(ua)` | `autumn-sdk-rust/<version> (...)` | |
| `debug_logging(bool)` | `AUTUMN_DEBUG` | Logs requests and responses at `debug` level. |
| `http_client(reqwest::Client)` | a new client | Bring your own proxy, TLS or pool settings. |

`Autumn::new(key)` and `Autumn::from_env()` are shortcuts for the builder with defaults.

## Environment variables

These are the same variables the JS SDK reads.

| Variable | Effect |
|---|---|
| `AUTUMN_SECRET_KEY` | Secret key when none is passed to the builder. |
| `AUTUMN_X_API_VERSION` | `x-api-version` header. |
| `AUTUMN_FAIL_OPEN` | `false` turns fail-open off. |
| `AUTUMN_DEBUG` | Any value except empty, `0` or `false` turns on debug logging. |

## Per-call options

`with_options` returns a client that applies `RequestOptions` to every call made through it. It is cheap, so use it inline:

```rust
use std::time::Duration;
use autumn_sdk::RequestOptions;

autumn
    .with_options(
        RequestOptions::new()
            .timeout(Duration::from_secs(30))
            .idempotency_key("order-1234")
            .header("x-request-id", "abc"),
    )
    .billing()
    .attach(params)
    .await?;
```

| Option | |
|---|---|
| `timeout(d)` | Overrides the client timeout for the call. |
| `retries(c)` / `retry_codes([..])` | Overrides the retry strategy and which status codes are retried (`"429"`, `"5XX"`, ...). |
| `server_url(url)` | Sends the call to another server. |
| `header(name, value)` | Adds a header. These override the SDK's own headers. |
| `idempotency_key(key)` | Sets `Idempotency-Key`. |

## Retries

Retries are off by default, as in the JS SDK. `RetryConfig::backoff()` turns them on with these defaults:

* It retries `429, 500, 502, 503, 504`.
* Waits grow exponentially from 500ms (exponent 1.5, at most 60s per wait), with up to 1s of random jitter.
* A `Retry-After` header, in seconds or as an HTTP date, takes precedence.
* It stops retrying after one hour in total.

To also retry network errors and timeouts, or to tune the timings:

```rust
use std::time::Duration;
use autumn_sdk::{BackoffStrategy, RetryConfig};

let retries = RetryConfig::Backoff {
    backoff: BackoffStrategy {
        initial_interval: Duration::from_millis(200),
        max_interval: Duration::from_secs(5),
        max_elapsed_time: Duration::from_secs(30),
        ..Default::default()
    },
    retry_connection_errors: true,
};
```

Retrying non-idempotent calls such as `track` can double-count usage. Send an idempotency key (`RequestOptions::idempotency_key`, or `idempotency_key` on batch events) when you turn retries on for writes.

## Logging

The SDK logs through the [`log`](https://docs.rs/log) crate under the target `autumn_sdk`:

* **`error`:** a call failed open (always logged).
* **`debug`:** with debug logging on, every request and response, including headers and bodies. The `Authorization` header is redacted.

Use any `log`-compatible logger, for example `env_logger` with `RUST_LOG=autumn_sdk=debug`, or `tracing` via `tracing-log`.
