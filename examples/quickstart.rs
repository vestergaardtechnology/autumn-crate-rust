//! Run with `AUTUMN_SECRET_KEY=am_sk_test_... cargo run --example quickstart`.

use autumn_sdk::models::{CheckParams, GetOrCreateCustomerParams, TrackParams};
use autumn_sdk::Autumn;

#[tokio::main]
async fn main() -> autumn_sdk::Result<()> {
    let autumn = Autumn::from_env();

    let customer = autumn
        .customers()
        .get_or_create(GetOrCreateCustomerParams::new("rust_example_user").name("Rust Example"))
        .await?;
    println!("customer: {}", customer.id.as_deref().unwrap_or("<none>"));

    let check = autumn
        .check(CheckParams::new("rust_example_user", "messages"))
        .await?;
    println!("allowed to send messages: {}", check.allowed);

    if check.allowed {
        let tracked = autumn
            .track(
                TrackParams::new("rust_example_user")
                    .feature_id("messages")
                    .value(1.0),
            )
            .await?;
        println!("tracked {} unit(s)", tracked.value);
    }
    Ok(())
}
