use crate::common::test_context::TestContext;
use truelayer_rust::apis::payments::Currency;

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
