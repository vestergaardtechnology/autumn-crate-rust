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
//! ## Layout
//!
//! | JS SDK                          | Rust SDK                                   |
//! |---------------------------------|--------------------------------------------|
//! | `new Autumn({ secretKey })`     | [`Autumn::new`] / [`Autumn::builder`]      |
//! | `autumn.check({...})`           | [`Autumn::check`]                          |
//! | `autumn.customers.getOrCreate`  | `autumn.customers().get_or_create(...)`    |
//! | `autumn.edge.check(...)`        | `autumn.edge().check(...)`                 |
//! | `{ retries, timeoutMs }` option | [`Autumn::with_options`] + [`RequestOptions`] |
//! | `AutumnError`                   | [`Error`] / [`ApiError`]                   |
//!
//! Request types live in [`models`] and follow the JS names (`CheckParams`,
//! `AttachParams`, ...). Each has a `new` constructor taking the required
//! fields and a chainable setter per optional field. Field names are the
//! snake_case wire names, so they match the REST API docs.
//!
//! ## Fail open
//!
//! As in the JS SDK, `check` and `track` fail open by default: if Autumn cannot
//! be reached, times out (5s default for these calls) or returns a 5XX, you
//! get an "allowed" response instead of an error. Turn it off with
//! [`AutumnBuilder::fail_open`] or `AUTUMN_FAIL_OPEN=false`.

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
