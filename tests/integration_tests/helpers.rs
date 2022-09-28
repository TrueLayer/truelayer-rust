use std::time::Duration;

use reqwest::Url;
use reqwest_retry::policies::ExponentialBackoff;
use truelayer_rust::{
    apis::payments::{
        AuthorizationFlowNextAction, BankTransferRequestBuilder, Beneficiary,
        ConsentSupportedBuilder, CreatePaymentRequestBuilder, CreatePaymentResponse,
        CreatePaymentUserRequest, Currency, NewUserBuilder, Payment, PaymentMethodRequest,
        PaymentStatus, PreselectedRequestBuilder, ProviderSelectionRequest,
        RedirectSupportedBuilder, StartAuthorizationFlowRequestBuilder,
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
        .create(
            &CreatePaymentRequestBuilder::default()
                .amount_in_minor(100)
                .currency(Currency::Gbp)
                .payment_method(PaymentMethodRequest::BankTransfer(
                    BankTransferRequestBuilder::default()
                        .provider_selection(ProviderSelectionRequest::Preselected(
                            PreselectedRequestBuilder::default()
                                .provider_id("mock-payments-gb-redirect".into())
                                .scheme_id("faster_payments_service".into())
                                .remitter(None)
                                .build()
                                .unwrap(),
                        ))
                        .beneficiary(Beneficiary::MerchantAccount {
                            merchant_account_id: ctx.merchant_account_gbp_id.clone(),
                            account_holder_name: None,
                        })
                        .build()
                        .unwrap(),
                ))
                .user(CreatePaymentUserRequest::NewUser(
                    NewUserBuilder::default()
                        .name(Some("someone".to_string()))
                        .email(Some("some.one@email.com".to_string()))
                        .phone(None)
                        .build()
                        .unwrap(),
                ))
                .metadata(None)
                .build()
                .unwrap(),
        )
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
            &StartAuthorizationFlowRequestBuilder::default()
                .provider_selection(None)
                .redirect(Some(
                    RedirectSupportedBuilder::default()
                        .return_uri(MOCK_RETURN_URI.to_string())
                        .direct_return_uri(None)
                        .build()
                        .unwrap(),
                ))
                .consent(Some(ConsentSupportedBuilder::default().build().unwrap()))
                .form(None)
                .build()
                .unwrap(),
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
