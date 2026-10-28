# autumn-sdk

Rust SDK for [Autumn](https://useautumn.com), the pricing and billing layer over Stripe.

It is a port of the server SDK in the official [`autumn-js`](https://www.npmjs.com/package/autumn-js) package. It has the same resources, operations, request and response shapes, defaults and runtime behaviour (fail-open, edge routing, retries). The models are generated from the JS SDK's own schemas, so they track the API exactly.

```toml
[dependencies]
autumn-sdk = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Quick start

```rust
use autumn_sdk::Autumn;
use autumn_sdk::models::{AttachParams, CheckParams, GetOrCreateCustomerParams, TrackParams};

#[tokio::main]
async fn main() -> autumn_sdk::Result<()> {
    // Reads AUTUMN_SECRET_KEY. Or: Autumn::new("am_sk_...")
    let autumn = Autumn::from_env();

    autumn
        .customers()
        .get_or_create(GetOrCreateCustomerParams::new("user_123").email("jane@example.com"))
        .await?;

    // Subscribe to a plan. `payment_url` is set when the customer must check out.
    let attach = autumn.billing().attach(AttachParams::new("user_123", "pro")).await?;
    if let Some(url) = attach.payment_url {
        println!("send the customer to {url}");
    }

    // Gate a feature, then record usage.
    let check = autumn.check(CheckParams::new("user_123", "messages")).await?;
    if check.allowed {
        autumn
            .track(TrackParams::new("user_123").feature_id("messages").value(1.0))
            .await?;
    }
    Ok(())
}
```

## Mapping from autumn-js

| autumn-js                                    | autumn-sdk                                              |
|----------------------------------------------|---------------------------------------------------------|
| `new Autumn({ secretKey })`                  | `Autumn::new(key)` / `Autumn::builder()...build()`      |
| `autumn.check({ customerId, featureId })`    | `autumn.check(CheckParams::new(customer_id, feature_id))` |
| `autumn.customers.getOrCreate({...})`        | `autumn.customers().get_or_create(...)`                 |
| `autumn.balances.finalize({...})`            | `autumn.balances().finalize(...)`                       |
| `autumn.edge.check({...})`                   | `autumn.edge().check(...)`                              |
| `autumn.check(req, { retries, timeoutMs })`  | `autumn.with_options(RequestOptions::new()...).check(req)` |
| `catch (e: AutumnError)`                     | `Err(autumn_sdk::Error::Api(e))`                        |

Every operation of the JS SDK is available (101 in total):

| Resource | Methods |
|---|---|
| *(root)* | `check`, `track`, `track_tokens`, `batch_track` |
| `customers()` | `get_or_create`, `get`, `list`, `update`, `delete`, `advance_test_clock`, `list_subscriptions`, `list_purchases` |
| `billing()` | `attach`, `create_schedule`, `multi_attach`, `preview_attach`, `preview_multi_attach`, `update`, `preview_update`, `multi_update`, `preview_multi_update`, `open_customer_portal`, `setup_payment`, `verify`, `import` |
| `balances()` | `create`, `list`, `update`, `delete`, `finalize` |
| `plans()` | `create`, `get`, `list`, `update`, `delete` |
| `features()` | `create`, `get`, `list`, `update`, `delete` |
| `entities()` | `create`, `get`, `list`, `update`, `delete` |
| `events()` | `list`, `aggregate` |
| `invoices()` | `create`, `finalize`, `insert`, `list`, `list_templates`, `pay`, `issue_credit_note`, `reissue`, `update`, `void` |
| `accounts()` | `get`, `update` |
| `logs()` | `search` |
| `licenses()` | `attach`, `release` |
| `referrals()` | `create_code`, `redeem_code`, `create_program`, `list_programs`, `get_program`, `update_program`, `delete_program` |
| `rewards()` | `create`, `list`, `get`, `update`, `delete`, `redeem_code` |
| `platform()` | `get_stripe_connection`, `disconnect_stripe`, `link_revenue_cat`, `sync_revenue_cat`, `get_revenue_cat_keys` |
| `keys()` | `mint`, `refresh`, `revoke` |
| `sandboxes()` | `create`, `list`, `delete`, `reset` |
| `webhooks()` | `create`, `get`, `list`, `update`, `delete`, `preview_sync`, `sync` |
| `approval_rules()` | `list`, `get`, `create`, `update`, `delete`, `list_versions`, `can` |

### Request and response types

Types in `autumn_sdk::models` keep their JS names (`CheckParams`, `AttachResponse`, ...). Fields use the API's snake_case wire names, so they match the REST docs.

* Request types have `new(required fields...)` plus a chainable setter for each optional field. The fields are public, so struct literals work too.
* Optional request fields that accept `null` are `Option<Option<T>>`: `None` omits the field, `Some(None)` sends `null` (to clear a value), and `Some(Some(v))` sends `v`.
* Enums that come back from the API are *open*. A value this SDK version doesn't know is kept as `Unknown(String)`, so new server-side values don't break deserialization.
* Approval-rule filters are a MongoDB-style query DSL. They are typed as `serde_json::Value` (`models::ApprovalRuleFilter`), so build them with `serde_json::json!`.

## Configuration

```rust
use std::time::Duration;
use autumn_sdk::{Autumn, RetryConfig};

let autumn = Autumn::builder()
    .secret_key("am_sk_...")              // default: AUTUMN_SECRET_KEY
    .x_api_version("2.6.0")               // default: AUTUMN_X_API_VERSION, else the generated API version
    .fail_open(true)                      // default: AUTUMN_FAIL_OPEN, else true
    .retry_config(RetryConfig::backoff()) // default: no retries
    .timeout(Duration::from_secs(10))     // default: 5s for check/track, none otherwise
    .edge_url("https://edge.example.com") // optional edge; use `.use_edge(true)` or `autumn.edge()`
    .debug_logging(true)                  // default: AUTUMN_DEBUG; logs via the `log` crate
    .build()?;
# Ok::<(), autumn_sdk::Error>(())
```

You can also pass your own `reqwest::Client` (`.http_client(...)`), override the server (`.server_url(...)`), or resolve the key per request with `.secret_key_provider(|| async { ... })`.

### Behaviour carried over from autumn-js

* **Fail open.** `check` and `track` return an "allowed" or zero-usage response instead of an error when Autumn is unreachable, times out, or returns a 5XX. This is on by default. See <https://docs.useautumn.com/documentation/fail-open>.
* **Default timeout.** `check` and `track` time out after 5 seconds unless you configure a timeout.
* **Edge routing.** With an `edge_url` and edge mode on, `check` and `balances().finalize` go to the edge first, with a 1s budget. They fall back to the API if the edge errors or returns a 5XX that it didn't forward. Write calls get an `Idempotency-Key` so the fallback can't double-count.
* **Retries.** Off by default. With `RetryConfig::Backoff`, the SDK retries `429, 500, 502, 503, 504` using exponential backoff with jitter, and honors `Retry-After`.

One difference: when fail-open is on, the JS SDK reports network failures on *every* operation as a synthetic `555` response. This crate only does that for `check` and `track`. Every other operation returns `Error::Connection` or `Error::Timeout`.

## Errors

```rust
# async fn run(autumn: autumn_sdk::Autumn) {
use autumn_sdk::{Error, models::GetCustomerParams};

match autumn.customers().get(GetCustomerParams::new("cus_123")).await {
    Ok(customer) => println!("{customer:?}"),
    Err(Error::Api(e)) if e.code.as_deref() == Some("customer_not_found") => println!("no such customer"),
    Err(e) => eprintln!("{e}"),
}
# }
```

## Scope

This crate covers the server SDK, which is the `autumn-js` root export. The package's other entry points are not ported:

* `autumn-js/react`, the UI hooks, which run in the browser.
* `autumn-js/better-auth`, a JS-only auth plugin.
* `autumn-js/backend` with its Express, Next and Hono adapters. Autumn has deprecated these and recommends calling the SDK from your own routes.

## Regenerating

`src/models` and `src/resources` are generated from a published `autumn-js` release (currently `autumn-js@1.5.28`, API version `2.6.0`):

```bash
./codegen/regenerate.sh          # latest
./codegen/regenerate.sh 1.5.28   # pinned
```

The generator (`codegen/`, Node 18+) parses the JS bundle's zod schemas and the type declarations without executing them. It emits Rust structs, enums and untagged unions with docs, plus one method per operation. The hand-written runtime lives in `src/client.rs`, `src/error.rs` and `src/util.rs`.

## License

MIT
