use crate::common::test_context::TestContext;
use truelayer_rust::apis::payments::{
    Beneficiary, CreatePaymentRequest, Currency, PaymentMethod, PaymentStatus, ProviderSelection,
    User,
};
use uuid::Uuid;

#[tokio::test]
async fn create_payment() {
    let ctx = TestContext::start().await;

    // Create a payment
    let merchant_account_id = Uuid::new_v4().to_string();
    let res = ctx
        .client
        .payments
        .create(&CreatePaymentRequest {
            amount_in_minor: 100,
            currency: Currency::Gbp,
            payment_method: PaymentMethod::BankTransfer {
                provider_selection: ProviderSelection::UserSelected { filter: None },
                beneficiary: Beneficiary::MerchantAccount {
                    merchant_account_id: merchant_account_id.clone(),
                    account_holder_name: None,
                },
            },
            user: User {
                id: None,
                name: Some("someone".to_string()),
                email: Some("some.one@email.com".to_string()),
                phone: None,
            },
        })
        .await
        .unwrap();

    // Assert that we got sensible values back
    assert!(!res.id.is_empty());
    assert!(!res.payment_token.is_empty());
    assert!(!res.user.id.is_empty());

    // Fetch the same payment
    let payment = ctx
        .client
        .payments
        .get_by_id(&res.id)
        .await
        .unwrap()
        .unwrap();

    // Ensure the returned payment contains correct data
    assert_eq!(payment.id, res.id);
    assert_eq!(payment.amount_in_minor, 100);
    assert_eq!(payment.currency, Currency::Gbp);
    assert_eq!(payment.user.id, Some(res.user.id));
    assert_eq!(payment.user.name.as_deref(), Some("someone"));
    assert_eq!(payment.user.email.as_deref(), Some("some.one@email.com"));
    assert_eq!(payment.user.phone, None);
    assert!(matches!(
        payment.payment_method,
        PaymentMethod::BankTransfer {
            beneficiary: Beneficiary::MerchantAccount {
                merchant_account_id: mid,
                ..
            },
            ..
        }
        if mid == merchant_account_id
    ));
    assert_eq!(payment.status, PaymentStatus::AuthorizationRequired);
}

#[tokio::test]
async fn fetch_non_existing_payment_returns_none() {
    let ctx = TestContext::start().await;

    let payment = ctx
        .client
        .payments
        .get_by_id(&Uuid::new_v4().to_string())
        .await
        .unwrap();

    assert!(payment.is_none());
}
