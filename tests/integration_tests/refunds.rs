use std::time::Duration;

use reqwest_retry::policies::ExponentialBackoff;
use truelayer_rust::{
    apis::payments::{CreateRefundRequest, PaymentStatus, RefundStatus},
    pollable::PollOptions,
    Pollable, PollableUntilTerminalState,
};

use crate::{common::test_context::TestContext, integration_tests::helpers};

#[tokio::test]
async fn create_refund() {
    let ctx = TestContext::start().await;

    // Prepare a closed-loop payment
    let payment = helpers::create_and_authorize_closed_loop_payment(&ctx)
        .await
        .unwrap();

    // Wait for payment to settle
    payment
        .poll_until(
            &ctx.client,
            PollOptions::default().with_retry_policy(
                ExponentialBackoff::builder()
                    .build_with_total_retry_duration(Duration::from_secs(20)),
            ),
            |payment| matches!(payment.status, PaymentStatus::Settled { .. }),
        )
        .await
        .unwrap();

    // Create a refund
    let res = ctx
        .client
        .payments
        .create_refund(
            &payment.id,
            &CreateRefundRequest {
                amount_in_minor: Some(payment.amount_in_minor),
                reference: "refund reference".into(),
                metadata: None,
            },
        )
        .await
        .unwrap()
        .unwrap();

    // Get refund
    let refund = (payment.id.as_str(), res)
        .poll_until_terminal_state(
            &ctx.client,
            PollOptions::default().with_retry_policy(
                ExponentialBackoff::builder()
                    .build_with_total_retry_duration(Duration::from_secs(60)),
            ),
        )
        .await
        .unwrap();
    assert!(matches!(refund.status, RefundStatus::Executed { .. }));

    // List refunds
    let refunds = ctx.client.payments.list_refunds(&payment.id).await.unwrap();
    assert!(refunds.iter().any(|r| r == &refund));
}
