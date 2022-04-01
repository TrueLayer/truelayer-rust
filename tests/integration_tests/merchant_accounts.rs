use crate::common::test_context::TestContext;
use chrono::{DateTime, Utc};
use rand::Rng;
use truelayer_rust::apis::{
    merchant_accounts::{
        ListPaymentSourcesRequest, ListTransactionsRequest, SetupSweepingRequest,
        SweepingFrequency, SweepingSettings, TransactionType,
    },
    payments::{AccountIdentifier, Currency},
};

#[tokio::test]
async fn list_merchant_accounts() {
    let ctx = TestContext::start().await;

    // List all the merchant accounts
    let merchant_accounts = ctx.client.merchant_accounts.list().await.unwrap();

    // Assert that we have at least one for GBP and one for EUR with positive balance
    for currency in [Currency::Gbp, Currency::Eur] {
        merchant_accounts
            .iter()
            .find(|m| m.currency == currency && m.available_balance_in_minor > 0)
            .unwrap();
    }
}

#[tokio::test]
async fn get_by_id_successful() {
    let ctx = TestContext::start().await;

    // Retrieve the details of the same merchant account we use to test payments
    let merchant_account = ctx
        .client
        .merchant_accounts
        .get_by_id(&ctx.merchant_account_gbp_id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(merchant_account.id, ctx.merchant_account_gbp_id);
    assert_eq!(merchant_account.currency, Currency::Gbp);
    assert!(merchant_account.available_balance_in_minor > 0);
    assert!(merchant_account.current_balance_in_minor > 0);
}

#[tokio::test]
async fn get_by_id_not_found() {
    let ctx = TestContext::start().await;

    // Retrieve the details of a non existent merchant account
    let merchant_account = ctx
        .client
        .merchant_accounts
        .get_by_id("non-existent-merchant-account")
        .await
        .unwrap();

    assert_eq!(merchant_account, None);
}

#[tokio::test]
async fn sweeping() {
    let ctx = TestContext::start().await;

    // Choose a random large amount (so that it doesn't trigger by mistake in sandbox)
    #[allow(clippy::inconsistent_digit_grouping)]
    let max_amount_in_minor = 10_000_000_00 + rand::thread_rng().gen_range(0..999_999_99);

    // Setup sweeping
    ctx.client
        .merchant_accounts
        .setup_sweeping(
            &ctx.merchant_account_gbp_id,
            &SetupSweepingRequest {
                max_amount_in_minor,
                currency: Currency::Gbp,
                frequency: SweepingFrequency::Fortnightly,
            },
        )
        .await
        .unwrap();

    // Retrieve the settings
    let settings = ctx
        .client
        .merchant_accounts
        .get_sweeping_settings(&ctx.merchant_account_gbp_id)
        .await
        .unwrap();
    assert_eq!(
        settings,
        Some(SweepingSettings {
            max_amount_in_minor,
            currency: Currency::Gbp,
            frequency: SweepingFrequency::Fortnightly,
            destination: AccountIdentifier::Iban {
                iban: ctx.merchant_account_gbp_sweeping_iban
            }
        })
    );

    // Disable sweeping
    ctx.client
        .merchant_accounts
        .disable_sweeping(&ctx.merchant_account_gbp_id)
        .await
        .unwrap();

    // Retrieve the settings again
    let settings = ctx
        .client
        .merchant_accounts
        .get_sweeping_settings(&ctx.merchant_account_gbp_id)
        .await
        .unwrap();
    assert_eq!(settings, None);
}

#[tokio::test]
async fn list_transactions() {
    let ctx = TestContext::start().await;

    // List the transactions of the account
    let transactions = ctx
        .client
        .merchant_accounts
        .list_transactions(
            &ctx.merchant_account_gbp_id,
            &ListTransactionsRequest {
                from: DateTime::parse_from_rfc3339("2021-03-01T00:00:00.000Z")
                    .unwrap()
                    .with_timezone(&Utc),
                to: DateTime::parse_from_rfc3339("2022-03-01T00:00:00.000Z")
                    .unwrap()
                    .with_timezone(&Utc),
                r#type: None,
            },
        )
        .await
        .unwrap();

    assert!(!transactions.is_empty());
}

#[tokio::test]
async fn list_payment_sources() {
    let ctx = TestContext::start().await;

    // Find an inbound transaction and extract its payment source
    let payment_source = ctx
        .client
        .merchant_accounts
        .list_transactions(
            &ctx.merchant_account_gbp_id,
            &ListTransactionsRequest {
                from: DateTime::parse_from_rfc3339("2021-03-01T00:00:00.000Z")
                    .unwrap()
                    .with_timezone(&Utc),
                to: DateTime::parse_from_rfc3339("2022-03-01T00:00:00.000Z")
                    .unwrap()
                    .with_timezone(&Utc),
                r#type: None,
            },
        )
        .await
        .unwrap()
        .into_iter()
        .filter_map(|t| match t.r#type {
            TransactionType::MerchantAccountPayment { payment_source, .. } => Some(payment_source),
            _ => None,
        })
        .next()
        .unwrap();

    // Fetch all the payment sources for the given user id
    let payment_sources = ctx
        .client
        .merchant_accounts
        .list_payment_sources(
            &ctx.merchant_account_gbp_id,
            &ListPaymentSourcesRequest {
                user_id: payment_source.user_id.unwrap(),
            },
        )
        .await
        .unwrap();

    assert!(!payment_sources.is_empty());
}
