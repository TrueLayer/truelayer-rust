use std::time::Duration;

use reqwest::Url;
use reqwest_retry::policies::ExponentialBackoff;
use truelayer_rust::{
    apis::payments::{
        AuthorizationFlowNextAction, Beneficiary, ConsentSupported, CreatePaymentRequest,
        CreatePaymentResponse, CreatePaymentUserRequest, Currency, Payment, PaymentMethodRequest,
        PaymentStatus, ProviderSelectionRequest, RedirectSupported, StartAuthorizationFlowRequest,
    },
    pollable::PollOptions,
    Pollable,
};

use crate::common::{test_context::TestContext, MockBankAction};

static MOCK_RETURN_URI: &str = "http://localhost:3000/callback";

pub async fn create_closed_loop_payment(
    ctx: &TestContext,
) -> anyhow::Result<CreatePaymentResponse> {
    let res = ctx
        .client
        .payments
        .create(&CreatePaymentRequest {
            amount_in_minor: 100,
            currency: Currency::Gbp,
            payment_method: PaymentMethodRequest::BankTransfer {
                provider_selection: ProviderSelectionRequest::Preselected {
                    provider_id: "mock-payments-gb-redirect".into(),
                    scheme_id: "faster_payments_service".into(),
                    remitter: None,
                },
                beneficiary: Beneficiary::MerchantAccount {
                    merchant_account_id: ctx.merchant_account_gbp_id.clone(),
                    account_holder_name: None,
                    reference: None,
                    statement_reference: None,
                },
            },
            user: CreatePaymentUserRequest::NewUser {
                name: Some("someone".to_string()),
                email: Some("some.one@email.com".to_string()),
                phone: None,
            },
            metadata: None,
        })
        .await?;
    Ok(res)
}

pub async fn create_and_authorize_closed_loop_payment(
    ctx: &TestContext,
) -> anyhow::Result<Payment> {
    let res = create_closed_loop_payment(ctx).await?;

    ctx.client
        .payments
        .start_authorization_flow(
            &res.id,
            &StartAuthorizationFlowRequest {
                provider_selection: None,
                redirect: Some(RedirectSupported {
                    return_uri: MOCK_RETURN_URI.to_string(),
                    direct_return_uri: None,
                }),
                consent: Some(ConsentSupported {}),
                form: None,
            },
        )
        .await?;

    let payment = ctx.client.payments.submit_consent(&res.id).await?;

    let redirect_uri = match payment
        .authorization_flow
        .ok_or_else(|| anyhow::anyhow!("Expected auth flow object"))?
        .actions
        .ok_or_else(|| anyhow::anyhow!("Expected actions"))?
        .next
    {
        AuthorizationFlowNextAction::Redirect { uri, .. } => Url::parse(&uri).unwrap(),
        _ => anyhow::bail!("Invalid payment state"),
    };

    let provider_return_uri = ctx
        .complete_mock_bank_redirect_authorization(&redirect_uri, MockBankAction::Execute)
        .await?;

    ctx.submit_provider_return_parameters(
        provider_return_uri.query().unwrap_or(""),
        provider_return_uri.fragment().unwrap_or(""),
    )
    .await?;

    let payment = res
        .poll_until(
            &ctx.client,
            PollOptions::default().with_retry_policy(
                ExponentialBackoff::builder()
                    .build_with_total_retry_duration(Duration::from_secs(20)),
            ),
            |payment| {
                matches!(
                    payment.status,
                    PaymentStatus::Failed { .. } | PaymentStatus::Settled { .. }
                )
            },
        )
        .await
        .unwrap();

    anyhow::ensure!(matches!(payment.status, PaymentStatus::Settled { .. }));

    Ok(payment)
}
