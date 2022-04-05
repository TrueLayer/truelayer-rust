use crate::common::test_context::TestContext;
use truelayer_rust::apis::{
    payments::{AccountIdentifier, Currency},
    payouts::{CreatePayoutRequest, PayoutBeneficiary},
};

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
    assert_eq!(
        payout.beneficiary,
        PayoutBeneficiary::ExternalAccount {
            account_holder_name: merchant_account.account_holder_name.clone(),
            account_identifier: account_identifier.clone(),
            reference: "rust-sdk-test".to_string(),
        }
    );
}
