# autumn-sdk

[![CI](https://github.com/vestergaardtechnology/autumn-crate-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/vestergaardtechnology/autumn-crate-rust/actions/workflows/ci.yml)
![MSRV 1.75](https://img.shields.io/badge/rustc-1.75+-blue.svg)
![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)

Rust SDK for [Autumn](https://useautumn.com), the pricing and billing layer over Stripe. Use it to model plans, gate features, meter usage and take payments from your Rust backend.

It is a port of the server SDK in the official [`autumn-js`](https://www.npmjs.com/package/autumn-js) package:

- **Complete.** Every one of the JS SDK's 101 operations: customers, billing, balances, plans, features, entities, events, invoices, referrals, rewards, webhooks and more.
- **Typed.** About 4,400 request and response types, generated from the JS SDK's own schemas, each field documented with Autumn's docs.
- **Behaves like the JS SDK.** Fail-open for `check` and `track`, a 5s default timeout for them, edge routing with automatic fallback, opt-in retries with backoff, and the same environment variables.
- **Async.** Built on Tokio and `reqwest` (rustls by default).

> Generated from `autumn-js@1.5.28` (Autumn API `2.6.0`). See [CONTRIBUTING](CONTRIBUTING.md) to regenerate it against a newer release.

## Install

```toml
[dependencies]
autumn-sdk = { git = "https://github.com/vestergaardtechnology/autumn-crate-rust" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

For the platform TLS stack instead of rustls, add `default-features = false, features = ["native-tls"]`.

## Quick start

```rust
use autumn_sdk::Autumn;
use autumn_sdk::models::{AttachParams, CheckParams, GetOrCreateCustomerParams, TrackParams};

#[tokio::main]
async fn main() -> autumn_sdk::Result<()> {
    // Reads AUTUMN_SECRET_KEY. Or: Autumn::new("am_sk_test_...")
    let autumn = Autumn::from_env();

    // 1. Make sure the customer exists (use your own user or org ID).
    autumn
        .customers()
        .get_or_create(GetOrCreateCustomerParams::new("user_123").email("jane@example.com"))
        .await?;

    // 2. Put them on a plan. `payment_url` is set when they need to check out.
    let attach = autumn.billing().attach(AttachParams::new("user_123", "pro")).await?;
    if let Some(url) = attach.payment_url {
        println!("send the customer to {url}");
    }

    // 3. Gate a feature, then record usage.
    let check = autumn.check(CheckParams::new("user_123", "messages")).await?;
    if check.allowed {
        autumn
            .track(TrackParams::new("user_123").feature_id("messages").value(1.0))
            .await?;
    }
    Ok(())
}
```

Request types take their required fields in `new(..)` and have a chainable setter for every optional field. Field names are the API's snake_case names, so they match [Autumn's API reference](https://docs.useautumn.com).

## Documentation

| | |
|---|---|
| [Getting started](docs/getting-started.md) | Install, authenticate, the core check / track / attach loop |
| [Configuration](docs/configuration.md) | Builder options, environment variables, per-call options, retries, logging |
| [Models](docs/models.md) | Request builders, nullable fields, enums, unions, approval-rule filters |
| [Errors](docs/errors.md) | `Error`, API error codes, validation errors |
| [Fail-open and edge routing](docs/fail-open-and-edge.md) | What `check`/`track` do when Autumn is slow or down |
| [Migrating from autumn-js](docs/migrating-from-js.md) | Side-by-side mapping of the JS API |
| API reference | `cargo doc --open`. Every method has a compiled example |

Runnable examples are in [`examples/`](examples): [`quickstart`](examples/quickstart.rs), [`billing`](examples/billing.rs), [`usage`](examples/usage.rs) and [`errors`](examples/errors.rs).

```bash
AUTUMN_SECRET_KEY=am_sk_test_... cargo run --example usage
```

## Operations

| Resource | Methods |
|---|---|
| *(client)* | `check`, `track`, `track_tokens`, `batch_track` |
| `customers()` | `get_or_create`, `get`, `list`, `update`, `delete`, `advance_test_clock`, `list_subscriptions`, `list_purchases` |
| `billing()` | `attach`, `preview_attach`, `multi_attach`, `preview_multi_attach`, `update`, `preview_update`, `multi_update`, `preview_multi_update`, `create_schedule`, `open_customer_portal`, `setup_payment`, `verify`, `import` |
| `balances()` | `create`, `list`, `update`, `delete`, `finalize` |
| `plans()` | `create`, `get`, `list`, `update`, `delete` |
| `features()` | `create`, `get`, `list`, `update`, `delete` |
| `entities()` | `create`, `get`, `list`, `update`, `delete` |
| `events()` | `list`, `aggregate` |
| `invoices()` | `create`, `finalize`, `insert`, `list`, `list_templates`, `pay`, `issue_credit_note`, `reissue`, `update`, `void` |
| `accounts()` | `get`, `update` |
| `licenses()` | `attach`, `release` |
| `logs()` | `search` |
| `referrals()` | `create_code`, `redeem_code`, `create_program`, `list_programs`, `get_program`, `update_program`, `delete_program` |
| `rewards()` | `create`, `list`, `get`, `update`, `delete`, `redeem_code` |
| `platform()` | `get_stripe_connection`, `disconnect_stripe`, `link_revenue_cat`, `sync_revenue_cat`, `get_revenue_cat_keys` |
| `keys()` | `mint`, `refresh`, `revoke` |
| `sandboxes()` | `create`, `list`, `delete`, `reset` |
| `webhooks()` | `create`, `get`, `list`, `update`, `delete`, `preview_sync`, `sync` |
| `approval_rules()` | `list`, `get`, `create`, `update`, `delete`, `list_versions`, `can` |

## Configuration at a glance

```rust
use std::time::Duration;
use autumn_sdk::{Autumn, RetryConfig};

let autumn = Autumn::builder()
    .secret_key("am_sk_live_...")         // default: AUTUMN_SECRET_KEY
    .fail_open(true)                      // default: AUTUMN_FAIL_OPEN, else true
    .retry_config(RetryConfig::backoff()) // default: no retries
    .timeout(Duration::from_secs(10))     // default: 5s for check/track, none otherwise
    .edge_url("https://edge.example.com") // optional; then autumn.edge().check(..)
    .build()?;
```

Errors are a single `autumn_sdk::Error` enum. API errors carry `status`, Autumn's error `code` (for example `customer_not_found`) and `message`. See [Errors](docs/errors.md).

## Scope

This crate ports the server SDK, which is the root export of `autumn-js`. It does not port the browser-only `autumn-js/react`, the JS-only `autumn-js/better-auth`, or the `autumn-js/backend` route handlers, which Autumn has deprecated in favour of calling the SDK from your own routes. Details are in [Migrating from autumn-js](docs/migrating-from-js.md#not-ported).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the layout, the code generator and the checks CI runs. Changes are listed in [CHANGELOG.md](CHANGELOG.md).

## License

MIT. Generated code includes documentation from `autumn-js` (MIT, © Recase Inc.). See [LICENSE](LICENSE).
