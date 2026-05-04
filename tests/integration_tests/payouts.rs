use std::time::Duration;

use crate::{
    common::{retry, test_context::TestContext},
    integration_tests::helpers,
};

use reqwest_retry::policies::ExponentialBackoff;
use truelayer_rust::{
    apis::{
        merchant_accounts::ListPaymentSourcesRequest,
        payments::{AccountIdentifier, Address, Currency, SubMerchants, UltimateCounterparty},
        payouts::{CreatePayoutRequest, PayoutBeneficiary, PayoutStatus},
    },
    pollable::PollOptions,
    PollableUntilTerminalState,
};

#[tokio::test]
async fn closed_loop_payout() {
    let ctx = TestContext::start().await;

    // Prepare a closed-loop payment
    let payment = helpers::create_and_authorize_closed_loop_payment(&ctx)
        .await
        .unwrap();

    // Create payout
    let payment_source = retry(Duration::from_secs(60), || async {
        ctx.client
            .merchant_accounts
            .list_payment_sources(
                &ctx.merchant_account_gbp_id,
                &ListPaymentSourcesRequest {
                    user_id: payment.user.id.clone(),
                },
            )
            .await
            .unwrap()
            .first()
            .cloned()
    })
    .await
    .expect("Payment source failed to appear");

    // Create a payout against this payment source
    let create_payout_response = ctx
        .client
        .payouts
        .create(&CreatePayoutRequest {
            merchant_account_id: ctx.merchant_account_gbp_id.clone(),
            amount_in_minor: 1,
            currency: Currency::Gbp,
            beneficiary: PayoutBeneficiary::PaymentSource {
                user_id: payment.user.id,
                payment_source_id: payment_source.id,
                reference: "rust-sdk-test".to_string(),
            },
            sub_merchants: None,
        })
        .await
        .unwrap();

    // Wait until the payout is executed
    let payout = create_payout_response
        .poll_until_terminal_state(
            &ctx.client,
            PollOptions::default().with_retry_policy(
                ExponentialBackoff::builder()
                    .build_with_total_retry_duration(Duration::from_secs(60)),
            ),
        )
        .await
        .unwrap();

    // Assert that it succeeded
    assert!(matches!(payout.status, PayoutStatus::Executed { .. }));
}

#[tokio::test]
async fn open_loop_payout() {
    let ctx = TestContext::start().await;

    // Get merchant account's first identifier
    let merchant_account = ctx
        .client
        .merchant_accounts
        .get_by_id(&ctx.merchant_account_gbp_id)
        .await
        .unwrap()
        .unwrap();

    // Use the IBAN identifier if the account has one because the GW currently supports only IBANs for payouts
    let account_identifier = merchant_account
        .account_identifiers
        .iter()
        .find(|id| matches!(id, AccountIdentifier::Iban { .. }))
        .unwrap_or_else(|| merchant_account.account_identifiers.first().unwrap());

    // Create a new payout
    let res = ctx
        .client
        .payouts
        .create(&CreatePayoutRequest {
            merchant_account_id: ctx.merchant_account_gbp_id.clone(),
            amount_in_minor: 1,
            currency: Currency::Gbp,
            beneficiary: PayoutBeneficiary::ExternalAccount {
                account_holder_name: merchant_account.account_holder_name.clone(),
                account_identifier: account_identifier.clone(),
                reference: "rust-sdk-test".to_string(),
            },
            sub_merchants: None,
        })
        .await
        .unwrap();

    assert!(!res.id.is_empty());

    // Retrieve it again
    let payout = ctx
        .client
        .payouts
        .get_by_id(&res.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(payout.id, res.id);
    assert_eq!(payout.merchant_account_id, ctx.merchant_account_gbp_id);
    assert_eq!(payout.amount_in_minor, 1);
    assert_eq!(payout.currency, Currency::Gbp);
    assert!(matches!(
        payout.beneficiary,
        PayoutBeneficiary::ExternalAccount {
            reference,
            ..
        } if reference == "rust-sdk-test"
    ));
}

#[tokio::test]
async fn open_loop_payout_with_sub_merchants() {
    let ctx = TestContext::start().await;

    // Get merchant account's first identifier
    let merchant_account = ctx
        .client
        .merchant_accounts
        .get_by_id(&ctx.merchant_account_gbp_id)
        .await
        .unwrap()
        .unwrap();

    let account_identifier = merchant_account
        .account_identifiers
        .iter()
        .find(|id| matches!(id, AccountIdentifier::Iban { .. }))
        .unwrap_or_else(|| merchant_account.account_identifiers.first().unwrap());

    // Create a new payout with sub_merchants
    let res = ctx
        .client
        .payouts
        .create(&CreatePayoutRequest {
            merchant_account_id: ctx.merchant_account_gbp_id.clone(),
            amount_in_minor: 1,
            currency: Currency::Gbp,
            beneficiary: PayoutBeneficiary::ExternalAccount {
                account_holder_name: merchant_account.account_holder_name.clone(),
                account_identifier: account_identifier.clone(),
                reference: "rust-sdk-test".to_string(),
            },
            sub_merchants: Some(SubMerchants {
                ultimate_counterparty: UltimateCounterparty::BusinessDivision {
                    id: "division-id".to_string(),
                    name: "Test Division".to_string(),
                },
            }),
        })
        .await
        .unwrap();

    assert!(!res.id.is_empty());

    // Retrieve it and verify
    let payout = ctx
        .client
        .payouts
        .get_by_id(&res.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(payout.id, res.id);
    assert_eq!(payout.merchant_account_id, ctx.merchant_account_gbp_id);
    assert_eq!(payout.amount_in_minor, 1);
    assert_eq!(payout.currency, Currency::Gbp);
}

#[tokio::test]
async fn open_loop_payout_with_business_client_sub_merchants() {
    let ctx = TestContext::start().await;

    let merchant_account = ctx
        .client
        .merchant_accounts
        .get_by_id(&ctx.merchant_account_gbp_id)
        .await
        .unwrap()
        .unwrap();

    let account_identifier = merchant_account
        .account_identifiers
        .iter()
        .find(|id| matches!(id, AccountIdentifier::Iban { .. }))
        .unwrap_or_else(|| merchant_account.account_identifiers.first().unwrap());

    let res = ctx
        .client
        .payouts
        .create(&CreatePayoutRequest {
            merchant_account_id: ctx.merchant_account_gbp_id.clone(),
            amount_in_minor: 1,
            currency: Currency::Gbp,
            beneficiary: PayoutBeneficiary::ExternalAccount {
                account_holder_name: merchant_account.account_holder_name.clone(),
                account_identifier: account_identifier.clone(),
                reference: "rust-sdk-test".to_string(),
            },
            sub_merchants: Some(SubMerchants {
                ultimate_counterparty: UltimateCounterparty::BusinessClient {
                    id: "client-id".to_string(),
                    trading_name: "Test Trading".to_string(),
                    commercial_name: Some("Test Commercial".to_string()),
                    url: Some("https://example.com".to_string()),
                    mcc: Some("5411".to_string()),
                    registration_number: None,
                    address: Some(Box::new(Address {
                        address_line1: "1 Test Street".to_string(),
                        address_line2: None,
                        city: "London".to_string(),
                        state: None,
                        zip: "EC1A 1BB".to_string(),
                        country_code: "GB".to_string(),
                    })),
                },
            }),
        })
        .await
        .unwrap();

    assert!(!res.id.is_empty());

    let payout = ctx
        .client
        .payouts
        .get_by_id(&res.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(payout.id, res.id);
    assert_eq!(payout.merchant_account_id, ctx.merchant_account_gbp_id);
    assert_eq!(payout.amount_in_minor, 1);
    assert_eq!(payout.currency, Currency::Gbp);
}
