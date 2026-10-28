//! Metering: check access, track usage, reserve balance with locks, record
//! LLM token usage and send events in bulk.
//!
//! ```bash
//! AUTUMN_SECRET_KEY=am_sk_test_... cargo run --example usage
//! ```

use autumn_sdk::models::{
    CheckParams, FinalizeBalanceParams, FinalizeLockAction, Lock, RequestBody, TrackParams,
    TrackTokensParams,
};
use autumn_sdk::Autumn;

#[tokio::main]
async fn main() -> autumn_sdk::Result<()> {
    let autumn = Autumn::from_env();
    let customer_id = "rust_example_user";

    // 1. Gate a feature, then record usage once the action succeeded.
    let check = autumn
        .check(CheckParams::new(customer_id, "messages"))
        .await?;
    if let Some(balance) = &check.balance {
        println!(
            "{} messages left (unlimited: {})",
            balance.remaining, balance.unlimited
        );
    }
    if check.allowed {
        autumn
            .track(
                TrackParams::new(customer_id)
                    .feature_id("messages")
                    .value(1.0),
            )
            .await?;
    }

    // 2. Check and consume in one atomic call.
    let res = autumn
        .check(
            CheckParams::new(customer_id, "messages")
                .required_balance(3.0)
                .send_event(true),
        )
        .await?;
    println!("consumed 3 messages: {}", res.allowed);

    // 3. Paywalls: ask for upgrade options when access is denied.
    let res = autumn
        .check(CheckParams::new(customer_id, "premium_reports").with_preview(true))
        .await?;
    if !res.allowed {
        println!("upgrade options: {:?}", res.preview);
    }

    // 4. Reserve units up front for a long-running job, then confirm or release.
    let lock = autumn
        .check(
            CheckParams::new(customer_id, "ai_credits")
                .required_balance(50.0)
                .lock(Lock::new("job_42")),
        )
        .await?;
    if lock.allowed {
        let job_succeeded = true;
        let action = if job_succeeded {
            FinalizeLockAction::Confirm
        } else {
            FinalizeLockAction::Release
        };
        autumn
            .balances()
            .finalize(FinalizeBalanceParams::new("job_42", action, customer_id))
            .await?;
    }

    // 5. LLM usage: Autumn prices the tokens with the model's pricing and markup.
    let tokens = autumn
        .track_tokens(TrackTokensParams::new(
            customer_id,
            "anthropic/claude-sonnet-4",
            1_000,
            500,
        ))
        .await?;
    println!("recorded ${} of AI usage", tokens.value);

    // 6. Up to 1000 events in one request, processed asynchronously.
    let events = (0..10)
        .map(|i| {
            RequestBody::new(customer_id)
                .feature_id("messages")
                .value(1.0)
                .idempotency_key(format!("import-{i}"))
        })
        .collect();
    autumn.batch_track(events).await?;
    Ok(())
}
