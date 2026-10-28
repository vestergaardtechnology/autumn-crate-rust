//! # autumn-sdk
//!
//! Rust SDK for [Autumn](https://useautumn.com), the pricing and billing layer
//! over Stripe. It is a port of the server side of the official
//! [`autumn-js`](https://www.npmjs.com/package/autumn-js) SDK: the same
//! resources, operations, request/response shapes, defaults and runtime
//! behaviour (fail-open, edge routing, retries), with Rust types generated
//! from the JS SDK's schemas.
//!
//! ## Quick start
//!
//! ```no_run
//! use autumn_sdk::Autumn;
//! use autumn_sdk::models::{AttachParams, CheckParams, GetOrCreateCustomerParams, TrackParams};
//!
//! # async fn run() -> autumn_sdk::Result<()> {
//! // Reads AUTUMN_SECRET_KEY, or use `Autumn::new("am_sk_...")`.
//! let autumn = Autumn::from_env();
//!
//! // Make sure the customer exists.
//! autumn
//!     .customers()
//!     .get_or_create(GetOrCreateCustomerParams::new("user_123").email("jane@example.com"))
//!     .await?;
//!
//! // Subscribe them to a plan; `payment_url` is set when they need to check out.
//! let attach = autumn.billing().attach(AttachParams::new("user_123", "pro")).await?;
//! println!("checkout: {:?}", attach.payment_url);
//!
//! // Gate a feature...
//! let check = autumn.check(CheckParams::new("user_123", "messages")).await?;
//! if check.allowed {
//!     // ...and record usage.
//!     autumn.track(TrackParams::new("user_123").feature_id("messages").value(1.0)).await?;
//! }
//! # Ok(()) }
//! ```
//!
//! ## Resources
//!
//! The four usage calls live on the client itself: [`Autumn::check`],
//! [`Autumn::track`], [`Autumn::track_tokens`] and [`Autumn::batch_track`].
//! Everything else is grouped by resource, as in the JS SDK:
//!
//! | Accessor | Resource |
//! |---|---|
//! | [`Autumn::customers`] | [`Customers`](resources::Customers): create, fetch, list, update and delete customers |
//! | [`Autumn::billing`] | [`Billing`](resources::Billing): attach plans, previews, upgrades, cancellations, checkout, customer portal |
//! | [`Autumn::balances`] | [`Balances`](resources::Balances): manual balances and finalizing locks |
//! | [`Autumn::plans`] / [`Autumn::features`] | [`Plans`](resources::Plans) / [`Features`](resources::Features): your pricing model |
//! | [`Autumn::entities`] | [`Entities`](resources::Entities): seats, workspaces and other per-entity balances |
//! | [`Autumn::events`] | [`Events`](resources::Events): usage event history and aggregation |
//! | [`Autumn::invoices`] | [`Invoices`](resources::Invoices) |
//! | [`Autumn::referrals`] / [`Autumn::rewards`] | [`Referrals`](resources::Referrals) / [`Rewards`](resources::Rewards) |
//! | [`Autumn::webhooks`] | [`Webhooks`](resources::Webhooks) |
//! | [`Autumn::accounts`], [`Autumn::licenses`], [`Autumn::logs`], [`Autumn::platform`], [`Autumn::keys`], [`Autumn::sandboxes`], [`Autumn::approval_rules`] | The remaining resources |
//!
//! Every method's documentation includes a compiled example.
//!
//! ## Requests and responses
//!
//! The types in [`models`] keep the JS SDK's names (`CheckParams`,
//! `AttachResponse`, ...). Request types take their required fields in
//! `new(..)` and have a chainable setter for each optional field. Field names
//! are the API's snake_case wire names. Optional request fields that accept
//! `null` are `Option<Option<T>>`, where `Some(None)` sends `null`. Response
//! enums keep unknown values in an `Unknown(String)` variant, so new
//! server-side values don't break you.
//!
//! ## Configuration
//!
//! [`Autumn::builder`] exposes every option of the JS SDK: secret key (or an
//! async key provider), server URL, API version, fail-open, edge URL,
//! retries, timeout, user agent, debug logging and a custom `reqwest`
//! client. Unset options fall back to `AUTUMN_SECRET_KEY`,
//! `AUTUMN_X_API_VERSION`, `AUTUMN_FAIL_OPEN` and `AUTUMN_DEBUG`. Per-call
//! overrides go through [`Autumn::with_options`] and [`RequestOptions`].
//!
//! ```no_run
//! use std::time::Duration;
//! use autumn_sdk::{Autumn, RequestOptions, RetryConfig};
//! use autumn_sdk::models::GetCustomerParams;
//!
//! # async fn run() -> autumn_sdk::Result<()> {
//! let autumn = Autumn::builder()
//!     .secret_key("am_sk_live_...")
//!     .retry_config(RetryConfig::backoff())
//!     .build()?;
//!
//! let customer = autumn
//!     .with_options(RequestOptions::new().timeout(Duration::from_secs(30)))
//!     .customers()
//!     .get(GetCustomerParams::new("user_123"))
//!     .await?;
//! # Ok(()) }
//! ```
//!
//! ## Errors
//!
//! All calls return [`Result`]. API errors are [`Error::Api`] and carry the
//! status, Autumn's error `code` and `message`:
//!
//! ```no_run
//! # async fn run(autumn: autumn_sdk::Autumn) -> autumn_sdk::Result<()> {
//! use autumn_sdk::{models::GetCustomerParams, Error};
//!
//! match autumn.customers().get(GetCustomerParams::new("user_123")).await {
//!     Ok(customer) => println!("{:?}", customer.email),
//!     Err(Error::Api(e)) if e.code.as_deref() == Some("customer_not_found") => {}
//!     Err(e) => return Err(e),
//! }
//! # Ok(()) }
//! ```
//!
//! ## Fail-open
//!
//! As in the JS SDK, `check` and `track` fail open by default. If Autumn
//! can't be reached, times out (5s by default for these calls) or returns a
//! 5XX, you get an "allowed" response instead of an error. Turn this off with
//! [`AutumnBuilder::fail_open`] or `AUTUMN_FAIL_OPEN=false`.
//!
//! ## Guides
//!
//! - [Getting started](https://github.com/vestergaardtechnology/autumn-crate-rust/blob/main/docs/getting-started.md)
//! - [Configuration](https://github.com/vestergaardtechnology/autumn-crate-rust/blob/main/docs/configuration.md)
//! - [Models](https://github.com/vestergaardtechnology/autumn-crate-rust/blob/main/docs/models.md)
//! - [Errors](https://github.com/vestergaardtechnology/autumn-crate-rust/blob/main/docs/errors.md)
//! - [Fail-open and edge routing](https://github.com/vestergaardtechnology/autumn-crate-rust/blob/main/docs/fail-open-and-edge.md)
//! - [Migrating from autumn-js](https://github.com/vestergaardtechnology/autumn-crate-rust/blob/main/docs/migrating-from-js.md)

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[macro_use]
mod util;

mod client;
mod error;
mod generated_meta;
pub mod models;
pub mod resources;

pub(crate) use client::Operation;
pub use client::{
    Autumn, AutumnBuilder, BackoffStrategy, RequestOptions, RetryConfig, DEFAULT_SERVER_URL,
};
pub use error::{ApiError, Error, ResponseValidationError, Result};
pub use generated_meta::{OPENAPI_DOC_VERSION, SOURCE_SDK_VERSION};
pub use util::{Lenient, UnknownVariant};

/// Re-exported so you don't need to depend on these crates directly.
pub use {reqwest, serde_json};
