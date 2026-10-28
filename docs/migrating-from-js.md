# Migrating from autumn-js

`autumn-sdk` mirrors the server SDK of `autumn-js` (`import { Autumn } from "autumn-js"`). Resources, operations, request and response shapes and runtime behaviour all match. This page maps one to the other.

## Client

| autumn-js | autumn-sdk |
|---|---|
| `new Autumn()` | `Autumn::from_env()` |
| `new Autumn({ secretKey })` | `Autumn::new(key)` |
| `new Autumn({ secretKey: async () => key })` | `Autumn::builder().secret_key_provider(\|\| async { key })` |
| `serverURL` | `.server_url(..)` |
| `xApiVersion` | `.x_api_version(..)` |
| `failOpen` | `.fail_open(..)` |
| `edgeURL` / `useEdge` | `.edge_url(..)` / `.use_edge(..)` |
| `retryConfig` | `.retry_config(RetryConfig::..)` |
| `timeoutMs` | `.timeout(Duration)` |
| `userAgent` | `.user_agent(..)` |
| `httpClient` | `.http_client(reqwest::Client)` |
| `debugLogger` / `AUTUMN_DEBUG` | `.debug_logging(true)` / `AUTUMN_DEBUG`, output through the `log` crate |
| `serverIdx` | not needed (there is a single server) |
| `autumn.edge` | `autumn.edge()` |

## Calls

Resources are methods instead of properties, and method names are snake_case:

| autumn-js | autumn-sdk |
|---|---|
| `await autumn.check({ customerId, featureId })` | `autumn.check(CheckParams::new(customer_id, feature_id)).await?` |
| `await autumn.track({ customerId, featureId, value: 3 })` | `autumn.track(TrackParams::new(customer_id).feature_id(feature_id).value(3.0)).await?` |
| `await autumn.trackTokens({...})` | `autumn.track_tokens(TrackTokensParams::new(..)).await?` |
| `await autumn.batchTrack([...])` | `autumn.batch_track(vec![RequestBody::new(..), ..]).await?` |
| `await autumn.customers.getOrCreate({...})` | `autumn.customers().get_or_create(..).await?` |
| `await autumn.billing.previewAttach({...})` | `autumn.billing().preview_attach(..).await?` |
| `await autumn.platform.linkRevenueCat({...})` | `autumn.platform().link_revenue_cat(..).await?` |
| `await autumn.features.list()` | `autumn.features().list().await?` |

Operations that take no body in JS (`features.list`, `rewards.list`, `keys.refresh`, `sandboxes.list`, ...) take no argument in Rust.

## Per-call options

| autumn-js | autumn-sdk |
|---|---|
| `autumn.check(req, { timeoutMs: 1000 })` | `autumn.with_options(RequestOptions::new().timeout(Duration::from_secs(1))).check(req)` |
| `{ retries: {...} }` | `RequestOptions::new().retries(RetryConfig::..)` |
| `{ retryCodes: ["5XX"] }` | `RequestOptions::new().retry_codes(["5XX"])` |
| `{ serverURL }` | `RequestOptions::new().server_url(..)` |
| `{ headers: {...} }` / `fetchOptions.headers` | `RequestOptions::new().header(k, v)` |
| `fetchOptions.signal` | drop the future, or wrap it in `tokio::time::timeout` |

## Request objects

* **Keys are snake_case.** `{ customerId, sendEvent: true }` becomes `CheckParams::new(..).send_event(true)`, or the field `send_event`.
* **Required fields go into `new`, optional fields have setters.** See [Models](models.md).
* **`null` vs. absent:** JS `{ name: null }` is Rust `params.name = Some(None)`.
* **Numbers:** JS `number` is `f64`, except fields the schema marks as integers (for example event timestamps and token counts), which are `i64`.

## Errors

| autumn-js | autumn-sdk |
|---|---|
| `AutumnError` / `AutumnDefaultError` | `Error::Api(ApiError)` (`status`, `code`, `message`, `body`, `headers`) |
| `ResponseValidationError` | `Error::ResponseValidation(..)` |
| `SDKValidationError` (input) | doesn't exist: the compiler checks inputs; `Error::Serialization` is practically unreachable |
| `RequestTimeoutError` | `Error::Timeout` |
| `ConnectionError` | `Error::Connection(..)` |
| `RequestAbortedError` | doesn't exist: dropping the future cancels the call |
| `InvalidRequestError` | `Error::InvalidRequest(..)` |

## Behaviour that is the same

* `check` and `track` fail open and default to a 5s timeout.
* Edge routing for `check` and `balances.finalize`, with automatic idempotency keys for write calls.
* No retries by default. With backoff on, the same retry codes, timings and `Retry-After` handling apply.
* The same environment variables: `AUTUMN_SECRET_KEY`, `AUTUMN_X_API_VERSION`, `AUTUMN_FAIL_OPEN` and `AUTUMN_DEBUG`.
* The same `x-api-version` (`2.6.0` for `autumn-js@1.5.28`).

## Behaviour that differs

* With fail-open on, the JS SDK reports network failures on every operation as a synthetic HTTP 555 error. Rust only fails `check` and `track` open, and returns `Error::Connection` or `Error::Timeout` for everything else.
* Approval-rule filters are `serde_json::Value` instead of thousands of generated types.
* Response unions that are identical per status code (the 200 vs. 202 bodies) are collapsed into one struct.

## Not ported

* **`autumn-js/react`**: browser UI hooks and components.
* **`autumn-js/better-auth`**: a JavaScript auth-library plugin.
* **`autumn-js/backend`, `/express`, `/next`, `/hono`, `/fetch`**: route handlers for the React hooks. Autumn deprecated them and recommends calling the SDK from your own server routes. With this crate that means a few lines in your Axum, Actix or other framework handler.
