# Getting started

## Install

```toml
[dependencies]
autumn-sdk = { git = "https://github.com/vestergaardtechnology/autumn-crate-rust" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

The client is async and runs on Tokio. It uses `reqwest` with rustls by default. To use the platform TLS stack instead, disable default features and enable `native-tls`.

## Authenticate

Create a secret key in the Autumn dashboard. Keys start with `am_sk_test_` (sandbox) or `am_sk_live_` (production). Then either pass the key explicitly:

```rust
use autumn_sdk::Autumn;

let autumn = Autumn::new("am_sk_test_...");
```

or set `AUTUMN_SECRET_KEY` and read it from the environment:

```rust
let autumn = Autumn::from_env();
```

`Autumn` is cheap to clone: clones share one connection pool. Build one client at startup and pass clones around, for example in your web framework's app state.

## The core loop

Most Autumn integrations need three calls. Customer IDs are your own user or organization IDs.

### 1. Make sure the customer exists

```rust
use autumn_sdk::models::GetOrCreateCustomerParams;

let customer = autumn
    .customers()
    .get_or_create(
        GetOrCreateCustomerParams::new("user_123")
            .name("Jane Doe")
            .email("jane@example.com"),
    )
    .await?;
```

### 2. Put them on a plan

```rust
use autumn_sdk::models::AttachParams;

let res = autumn
    .billing()
    .attach(AttachParams::new("user_123", "pro").success_url("https://example.com/welcome"))
    .await?;

if let Some(url) = res.payment_url {
    // No card on file yet: redirect the user to Stripe Checkout.
}
```

Free plans and customers who already have a payment method are attached right away, and `payment_url` is `None`.

### 3. Gate features and record usage

```rust
use autumn_sdk::models::{CheckParams, TrackParams};

let check = autumn.check(CheckParams::new("user_123", "messages")).await?;
if !check.allowed {
    // Show a paywall. Pass `.with_preview(true)` to get upgrade options in `check.preview`.
}

// ...do the work, then record it:
autumn
    .track(TrackParams::new("user_123").feature_id("messages").value(1.0))
    .await?;
```

To check and deduct atomically in one request, use `CheckParams::new(..).required_balance(n).send_event(true)`.

## Where to go next

* [`examples/usage.rs`](../examples/usage.rs) covers locks, LLM token tracking and batch tracking.
* [`examples/billing.rs`](../examples/billing.rs) covers previews, cancellation and the customer portal.
* [Configuration](configuration.md) covers retries, timeouts and logging.
* [Fail-open and edge routing](fail-open-and-edge.md) explains what `check` returns when Autumn is down. Read it before production.
