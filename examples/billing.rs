//! Subscriptions and checkout: preview, attach, change plans, cancel, and open
//! the Stripe customer portal.
//!
//! ```bash
//! AUTUMN_SECRET_KEY=am_sk_test_... cargo run --example billing
//! ```

use autumn_sdk::models::{
    AttachFeatureQuantity, AttachParams, BillingUpdateCancelAction, GetOrCreateCustomerParams,
    OpenCustomerPortalParams, PreviewAttachParams, UpdateSubscriptionParams,
};
use autumn_sdk::Autumn;

#[tokio::main]
async fn main() -> autumn_sdk::Result<()> {
    let autumn = Autumn::from_env();
    let customer_id = "rust_example_user";

    autumn
        .customers()
        .get_or_create(GetOrCreateCustomerParams::new(customer_id).email("jane@example.com"))
        .await?;

    // Show the customer what they will pay before committing.
    let preview = autumn
        .billing()
        .preview_attach(PreviewAttachParams::new(customer_id, "pro"))
        .await?;
    println!(
        "pro costs {} {} today ({} line items)",
        preview.total,
        preview.currency,
        preview.line_items.len()
    );

    // Attach the plan. With no payment method on file, Autumn returns a
    // Stripe Checkout URL to send the customer to.
    let attach = autumn
        .billing()
        .attach(
            AttachParams::new(customer_id, "pro")
                .success_url("https://example.com/billing/success")
                // Prepaid features take a quantity, e.g. 5 seats.
                .feature_quantities(vec![AttachFeatureQuantity::new("seats").quantity(5.0)]),
        )
        .await?;
    match attach.payment_url {
        Some(url) => println!("redirect the customer to {url}"),
        None => println!("plan attached"),
    }

    // Cancel at the end of the billing cycle (use `Uncancel` to undo).
    autumn
        .billing()
        .update(
            UpdateSubscriptionParams::new(customer_id)
                .plan_id("pro")
                .cancel_action(BillingUpdateCancelAction::CancelEndOfCycle),
        )
        .await?;

    // Let the customer manage payment methods and invoices themselves.
    let portal = autumn
        .billing()
        .open_customer_portal(
            OpenCustomerPortalParams::new(customer_id).return_url("https://example.com/account"),
        )
        .await?;
    println!("customer portal: {}", portal.url);
    Ok(())
}
