//! Error handling, per-call options and client configuration.
//!
//! ```bash
//! AUTUMN_SECRET_KEY=am_sk_test_... cargo run --example errors
//! ```

use std::time::Duration;

use autumn_sdk::models::{GetCustomerParams, UpdateCustomerParams};
use autumn_sdk::{Autumn, Error, RequestOptions, RetryConfig};

#[tokio::main]
async fn main() -> autumn_sdk::Result<()> {
    let autumn = Autumn::builder()
        .retry_config(RetryConfig::backoff())
        .timeout(Duration::from_secs(15))
        .build()?;

    match autumn
        .customers()
        .get(GetCustomerParams::new("does_not_exist"))
        .await
    {
        Ok(customer) => println!("found {:?}", customer.name),
        Err(Error::Api(e)) if e.code.as_deref() == Some("customer_not_found") => {
            println!("no such customer");
        }
        Err(Error::Api(e)) => println!("API error {} ({:?}): {:?}", e.status, e.code, e.message),
        Err(Error::Timeout) => println!("timed out"),
        Err(e) => return Err(e),
    }

    // Per-call overrides: a longer timeout and an explicit idempotency key.
    let mut update = UpdateCustomerParams::new("rust_example_user").email("new@example.com");
    // `Some(None)` sends `null`, which clears the field.
    update.name = Some(None);
    autumn
        .with_options(
            RequestOptions::new()
                .timeout(Duration::from_secs(30))
                .idempotency_key("update-rust-example-user-1"),
        )
        .customers()
        .update(update)
        .await?;
    Ok(())
}
