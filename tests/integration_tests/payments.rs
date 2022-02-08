use crate::common::test_context::TestContext;
use truelayer_rust::apis::payments::{
    AuthorizationFlow, AuthorizationFlowActions, AuthorizationFlowNextAction,
    AuthorizationFlowResponseStatus, Beneficiary, CreatePaymentRequest, Currency, PaymentMethod,
    PaymentStatus, ProviderSelection, ProviderSelectionSupported, RedirectSupported,
    StartAuthorizationFlowRequest, SubmitProviderSelectionActionRequest, User,
};
use uuid::Uuid;

static MOCK_PROVIDER_ID: &str = "mock-payments-gb-redirect";
static MOCK_RETURN_URI: &str = "http://localhost:3000/callback";

#[tokio::test]
async fn create_payment() {
    let ctx = TestContext::start().await;

    // Create a payment
    let res = ctx
        .client
        .payments
        .create(&CreatePaymentRequest {
            amount_in_minor: 100,
            currency: Currency::Gbp,
            payment_method: PaymentMethod::BankTransfer {
                provider_selection: ProviderSelection::UserSelected { filter: None },
                beneficiary: Beneficiary::MerchantAccount {
                    merchant_account_id: ctx.merchant_account_gbp_id.clone(),
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
    assert!(!res.resource_token.is_empty());
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
                merchant_account_id,
                ..
            },
            ..
        }
        if merchant_account_id == ctx.merchant_account_gbp_id
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

#[tokio::test]
async fn hpp_link_returns_200() {
    let ctx = TestContext::start().await;

    // Create a payment
    let res = ctx
        .client
        .payments
        .create(&CreatePaymentRequest {
            amount_in_minor: 100,
            currency: Currency::Gbp,
            payment_method: PaymentMethod::BankTransfer {
                provider_selection: ProviderSelection::UserSelected { filter: None },
                beneficiary: Beneficiary::MerchantAccount {
                    merchant_account_id: ctx.merchant_account_gbp_id.clone(),
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

    // Get the HPP link for it
    let hpp_url = ctx
        .client
        .payments
        .get_hosted_payments_page_link(&res.id, &res.resource_token, MOCK_RETURN_URI)
        .await;

    // Make a request and assert we get back a 200
    assert!(reqwest::Client::new()
        .get(hpp_url)
        .header(
            reqwest::header::USER_AGENT,
            concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"))
        )
        .send()
        .await
        .unwrap()
        .status()
        .is_success());
}

#[tokio::test]
async fn complete_authorization_flow_with_user_selected_provider() {
    let ctx = TestContext::start().await;

    // Create a payment
    let res = ctx
        .client
        .payments
        .create(&CreatePaymentRequest {
            amount_in_minor: 100,
            currency: Currency::Gbp,
            payment_method: PaymentMethod::BankTransfer {
                provider_selection: ProviderSelection::UserSelected { filter: None },
                beneficiary: Beneficiary::MerchantAccount {
                    merchant_account_id: ctx.merchant_account_gbp_id.clone(),
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

    // Retrieve the payment by id and check its status
    assert_eq!(
        ctx.client
            .payments
            .get_by_id(&res.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        PaymentStatus::AuthorizationRequired
    );

    // Start authorization flow
    let start_auth_flow_response = ctx
        .client
        .payments
        .start_authorization_flow(
            &res.id,
            &StartAuthorizationFlowRequest {
                provider_selection: Some(ProviderSelectionSupported {}),
                redirect: Some(RedirectSupported {
                    return_uri: MOCK_RETURN_URI.to_string(),
                }),
            },
        )
        .await
        .unwrap();

    assert_eq!(
        start_auth_flow_response.status,
        AuthorizationFlowResponseStatus::Authorizing
    );
    assert!(matches!(
        start_auth_flow_response.authorization_flow,
        Some(AuthorizationFlow {
            actions: Some(AuthorizationFlowActions {
                next: AuthorizationFlowNextAction::ProviderSelection { .. }
            }),
            ..
        })
    ));

    // Retrieve the payment by id and check its status
    assert!(matches!(
        ctx.client
            .payments
            .get_by_id(&res.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        PaymentStatus::Authorizing {
            authorization_flow: AuthorizationFlow {
                actions: Some(AuthorizationFlowActions {
                    next: AuthorizationFlowNextAction::ProviderSelection { .. }
                }),
                ..
            }
        }
    ));

    // Submit provider selection
    let submit_provider_selection_response = ctx
        .client
        .payments
        .submit_provider_selection(
            &res.id,
            &SubmitProviderSelectionActionRequest {
                provider_id: MOCK_PROVIDER_ID.to_string(),
            },
        )
        .await
        .unwrap();

    assert_eq!(
        submit_provider_selection_response.status,
        AuthorizationFlowResponseStatus::Authorizing
    );
    assert!(matches!(
        submit_provider_selection_response.authorization_flow,
        Some(AuthorizationFlow {
            actions: Some(AuthorizationFlowActions {
                next: AuthorizationFlowNextAction::Redirect { .. }
            }),
            ..
        })
    ));

    // Retrieve the payment by id and check its status
    assert!(matches!(
        ctx.client
            .payments
            .get_by_id(&res.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        PaymentStatus::Authorizing {
            authorization_flow: AuthorizationFlow {
                actions: Some(AuthorizationFlowActions {
                    next: AuthorizationFlowNextAction::Redirect { .. }
                }),
                ..
            }
        }
    ));
}

#[tokio::test]
async fn complete_authorization_flow_with_preselected_provider() {
    let ctx = TestContext::start().await;

    // Create a payment
    let res = ctx
        .client
        .payments
        .create(&CreatePaymentRequest {
            amount_in_minor: 100,
            currency: Currency::Gbp,
            payment_method: PaymentMethod::BankTransfer {
                provider_selection: ProviderSelection::Preselected {
                    provider_id: MOCK_PROVIDER_ID.to_string(),
                    scheme_id: "faster_payments_service".to_string(),
                    remitter: None,
                },
                beneficiary: Beneficiary::MerchantAccount {
                    merchant_account_id: ctx.merchant_account_gbp_id.clone(),
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

    // Retrieve the payment by id and check its status
    assert_eq!(
        ctx.client
            .payments
            .get_by_id(&res.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        PaymentStatus::AuthorizationRequired
    );

    // Start authorization flow
    let start_auth_flow_response = ctx
        .client
        .payments
        .start_authorization_flow(
            &res.id,
            &StartAuthorizationFlowRequest {
                provider_selection: Some(ProviderSelectionSupported {}),
                redirect: Some(RedirectSupported {
                    return_uri: MOCK_RETURN_URI.to_string(),
                }),
            },
        )
        .await
        .unwrap();

    assert_eq!(
        start_auth_flow_response.status,
        AuthorizationFlowResponseStatus::Authorizing
    );
    assert!(matches!(
        start_auth_flow_response.authorization_flow,
        Some(AuthorizationFlow {
            actions: Some(AuthorizationFlowActions {
                next: AuthorizationFlowNextAction::Redirect { .. }
            }),
            ..
        })
    ));

    // Retrieve the payment by id and check its status
    assert!(matches!(
        ctx.client
            .payments
            .get_by_id(&res.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        PaymentStatus::Authorizing {
            authorization_flow: AuthorizationFlow {
                actions: Some(AuthorizationFlowActions {
                    next: AuthorizationFlowNextAction::Redirect { .. }
                }),
                ..
            }
        }
    ));
}
