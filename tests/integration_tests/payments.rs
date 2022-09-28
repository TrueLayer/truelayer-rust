use crate::{
    common::{test_context::TestContext, MockBankAction},
    integration_tests::helpers,
};
use retry_policies::policies::ExponentialBackoff;
use std::{collections::HashMap, time::Duration};
use test_case::test_case;
use truelayer_rust::{
    apis::payments::{
        AccountIdentifier, AdditionalInputType, AuthorizationFlow, AuthorizationFlowActions,
        AuthorizationFlowNextAction, AuthorizationFlowResponseStatus, BankTransferRequestBuilder,
        Beneficiary, CreatePaymentRequestBuilder, CreatePaymentStatus, CreatePaymentUserRequest,
        Currency, FailureStage, FormSupportedBuilder, NewUser, PaymentMethodRequest, PaymentStatus,
        PreselectedRequestBuilder, ProviderSelectionRequest, ProviderSelectionSupportedBuilder,
        RedirectSupportedBuilder, SchemeSelection, StartAuthorizationFlowRequestBuilder,
        StartAuthorizationFlowResponse, SubmitFormActionRequestBuilder,
        SubmitProviderReturnParametersRequestBuilder,
        SubmitProviderReturnParametersResponseResource,
        SubmitProviderSelectionActionRequestBuilder, UserSelectedRequestBuilder,
    },
    pollable::PollOptions,
    PollableUntilTerminalState,
};
use url::Url;
use uuid::Uuid;

static MOCK_PROVIDER_ID_REDIRECT: &str = "mock-payments-gb-redirect";
static MOCK_PROVIDER_ID_ADDITIONAL_INPUTS: &str = "mock-payments-de-redirect-additional-input-text";
static MOCK_RETURN_URI: &str = "http://localhost:3000/callback";

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
    let create_payment_request = CreatePaymentRequestBuilder::default()
        .amount_in_minor(1)
        .currency(Currency::Gbp)
        .payment_method(PaymentMethodRequest::BankTransfer(
            BankTransferRequestBuilder::default()
                .provider_selection(ProviderSelectionRequest::UserSelected(
                    UserSelectedRequestBuilder::default().build().unwrap(),
                ))
                .beneficiary(Beneficiary::MerchantAccount {
                    merchant_account_id: ctx.merchant_account_gbp_id.clone(),
                    account_holder_name: None,
                })
                .build()
                .unwrap(),
        ))
        .user(CreatePaymentUserRequest::NewUser(NewUser {
            name: Some("someone".to_string()),
            email: Some("some.one@email.com".to_string()),
            phone: None,
        }))
        .build()
        .unwrap();
    let res = ctx
        .client
        .payments
        .create(&create_payment_request)
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

#[derive(Clone, Debug, Eq, PartialEq)]
enum ScenarioProviderSelection {
    UserSelected {
        provider_id: String,
    },
    Preselected {
        provider_id: String,
        scheme_id: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ScenarioBeneficiary {
    ClosedLoop,
    OpenLoop {
        account_identifier: AccountIdentifier,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ScenarioExpectedStatus {
    ExecutedOrSettled,
    Failed {
        failure_stage: FailureStage,
        failure_reason: &'static str,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RedirectFlow {
    Classic,
    DirectReturn,
}

struct CreatePaymentScenario {
    currency: Currency,
    beneficiary: ScenarioBeneficiary,
    provider_selection: ScenarioProviderSelection,
    mock_bank_action: MockBankAction,
    expected_status: ScenarioExpectedStatus,
    redirect_flow: RedirectFlow,
}

impl CreatePaymentScenario {
    async fn run(&self) {
        let ctx = TestContext::start().await;

        let provider_selection = match &self.provider_selection {
            ScenarioProviderSelection::UserSelected { .. } => {
                ProviderSelectionRequest::UserSelected(
                    UserSelectedRequestBuilder::default()
                        .scheme_selection(Some(SchemeSelection::InstantPreferred {
                            allow_remitter_fee: Some(true),
                        }))
                        .build()
                        .unwrap(),
                )
            }
            ScenarioProviderSelection::Preselected {
                provider_id,
                scheme_id,
            } => {
                let provider = ctx
                    .client
                    .payments_providers
                    .get_by_id(provider_id)
                    .await
                    .unwrap()
                    .unwrap();
                let available_schemes = provider
                    .capabilities
                    .payments
                    .bank_transfer
                    .unwrap()
                    .schemes;

                assert!(available_schemes.iter().any(|s| &s.id == scheme_id));

                ProviderSelectionRequest::Preselected(
                    PreselectedRequestBuilder::default()
                        .provider_id(provider_id.clone())
                        .scheme_id(scheme_id.clone())
                        .build()
                        .unwrap(),
                )
            }
        };

        // Create a payment
        let create_payment_request = CreatePaymentRequestBuilder::default()
            .amount_in_minor(1)
            .currency(self.currency.clone())
            .payment_method(PaymentMethodRequest::BankTransfer(
                BankTransferRequestBuilder::default()
                    .provider_selection(provider_selection)
                    .beneficiary(match self.beneficiary {
                        ScenarioBeneficiary::ClosedLoop => Beneficiary::MerchantAccount {
                            merchant_account_id: ctx.merchant_account_gbp_id.clone(),
                            account_holder_name: None,
                        },
                        ScenarioBeneficiary::OpenLoop {
                            ref account_identifier,
                        } => Beneficiary::ExternalAccount {
                            account_holder_name: "Account Holder".to_string(),
                            account_identifier: account_identifier.clone(),
                            reference: "Reference".to_string(),
                        },
                    })
                    .build()
                    .unwrap(),
            ))
            .user(CreatePaymentUserRequest::NewUser(NewUser {
                name: Some("someone".to_string()),
                email: Some("some.one@email.com".to_string()),
                phone: None,
            }))
            .metadata(Some(HashMap::from([("some".into(), "metadata".into())])))
            .build()
            .unwrap();
        let res = ctx
            .client
            .payments
            .create(&create_payment_request)
            .await
            .unwrap();

        // Assert that we got sensible values back
        assert!(!res.id.is_empty());
        assert!(!res.resource_token.expose_secret().is_empty());
        assert!(!res.user.id.is_empty());
        assert_eq!(res.status, CreatePaymentStatus::AuthorizationRequired);

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
        assert_eq!(payment.amount_in_minor, 1);
        assert_eq!(payment.currency, self.currency);
        assert_eq!(payment.user.id, res.user.id);
        assert_eq!(payment.user.name.as_deref(), Some("someone"));
        assert_eq!(payment.user.email.as_deref(), Some("some.one@email.com"));
        assert_eq!(payment.user.phone, None);
        assert_eq!(
            PaymentMethodRequest::from(payment.payment_method),
            create_payment_request.payment_method
        );
        assert_eq!(payment.status, PaymentStatus::AuthorizationRequired);
        assert_eq!(
            payment.metadata.unwrap().get("some"),
            Some(&"metadata".into())
        );

        // Start authorization flow
        let start_authorization_flow_request = StartAuthorizationFlowRequestBuilder::default()
            .provider_selection(Some(
                ProviderSelectionSupportedBuilder::default()
                    .build()
                    .unwrap(),
            ))
            .redirect(Some(
                RedirectSupportedBuilder::default()
                    .return_uri(MOCK_RETURN_URI.to_string())
                    .direct_return_uri(
                        (self.redirect_flow == RedirectFlow::DirectReturn)
                            .then(|| MOCK_RETURN_URI.to_string()),
                    )
                    .build()
                    .unwrap(),
            ))
            .form(Some(
                FormSupportedBuilder::default()
                    .input_types(vec![
                        AdditionalInputType::Text,
                        AdditionalInputType::Select,
                        AdditionalInputType::TextWithImage,
                    ])
                    .build()
                    .unwrap(),
            ))
            .build()
            .unwrap();
        let StartAuthorizationFlowResponse {
            mut authorization_flow,
            mut status,
        } = ctx
            .client
            .payments
            .start_authorization_flow(&res.id, &start_authorization_flow_request)
            .await
            .unwrap();

        if let ScenarioProviderSelection::UserSelected { provider_id } = &self.provider_selection {
            // Assert that the next action in the auth flow is ProviderSelection
            assert_eq!(status, AuthorizationFlowResponseStatus::Authorizing);
            assert!(matches!(
                authorization_flow,
                Some(AuthorizationFlow {
                    actions: Some(AuthorizationFlowActions {
                        next: AuthorizationFlowNextAction::ProviderSelection { providers, .. }
                    }),
                    ..
                })
                if !providers.is_empty()
            ));

            // Retrieve the payment by id and re-check its status
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
                            next: AuthorizationFlowNextAction::ProviderSelection { providers, .. }
                        }),
                        ..
                    }
                }
                if !providers.is_empty()
            ));

            // Submit provider selection
            let submit_provider_selection_response = ctx
                .client
                .payments
                .submit_provider_selection(
                    &res.id,
                    &SubmitProviderSelectionActionRequestBuilder::default()
                        .provider_id(provider_id.to_string())
                        .build()
                        .unwrap(),
                )
                .await
                .unwrap();

            status = submit_provider_selection_response.status;
            authorization_flow = submit_provider_selection_response.authorization_flow;
        }

        // Assert that the next action in the auth flow is Consent
        assert_eq!(status, AuthorizationFlowResponseStatus::Authorizing);
        assert!(matches!(
            authorization_flow,
            Some(AuthorizationFlow {
                actions: Some(AuthorizationFlowActions {
                    next: AuthorizationFlowNextAction::Consent { .. }
                }),
                ..
            })
        ));

        // Submit consent
        let submit_consent_response = ctx.client.payments.submit_consent(&res.id).await.unwrap();

        status = submit_consent_response.status;
        authorization_flow = submit_consent_response.authorization_flow;

        if match &self.provider_selection {
            ScenarioProviderSelection::Preselected { provider_id, .. } => provider_id,
            ScenarioProviderSelection::UserSelected { provider_id } => provider_id,
        } == MOCK_PROVIDER_ID_ADDITIONAL_INPUTS
        {
            // Assert that the next action in the auth flow is Form
            assert_eq!(status, AuthorizationFlowResponseStatus::Authorizing);
            assert!(matches!(
                authorization_flow,
                Some(AuthorizationFlow {
                    actions: Some(AuthorizationFlowActions {
                        next: AuthorizationFlowNextAction::Form { inputs, .. }
                    }),
                    ..
                })
                if !inputs.is_empty()
            ));

            // Retrieve the payment by id and re-check its status
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
                            next: AuthorizationFlowNextAction::Form { inputs, .. }
                        }),
                        ..
                    }
                }
                if !inputs.is_empty()
            ));

            // Submit form submission
            let submit_form_response = ctx
                .client
                .payments
                .submit_form_inputs(
                    &res.id,
                    &SubmitFormActionRequestBuilder::default()
                        .inputs(HashMap::from([
                            ("psu-branch-code".to_string(), "123".to_string()),
                            ("psu-account-number".to_string(), "1234567".to_string()),
                            ("psu-sub-account".to_string(), "01".to_string()),
                        ]))
                        .build()
                        .unwrap(),
                )
                .await
                .unwrap();

            status = submit_form_response.status;
            authorization_flow = submit_form_response.authorization_flow;
        }

        // Assert that the next action in the auth flow is Redirect
        assert_eq!(status, AuthorizationFlowResponseStatus::Authorizing);
        assert!(matches!(
            authorization_flow,
            Some(AuthorizationFlow {
                actions: Some(AuthorizationFlowActions {
                    next: AuthorizationFlowNextAction::Redirect { uri, .. }
                }),
                ..
            })
            if Url::parse(&uri).is_ok()
        ));

        // Retrieve the payment by id and re-check its status.
        // Also extract the redirect uri because it will be needed to drive the payment to completion.
        let payment = ctx
            .client
            .payments
            .get_by_id(&res.id)
            .await
            .unwrap()
            .unwrap();
        let redirect_uri = match payment.status {
            PaymentStatus::Authorizing {
                authorization_flow:
                    AuthorizationFlow {
                        actions:
                            Some(AuthorizationFlowActions {
                                next: AuthorizationFlowNextAction::Redirect { ref uri, .. },
                            }),
                        ..
                    },
            } => Url::parse(uri).unwrap(),
            _ => panic!("Invalid payment state"),
        };

        // Drive the payment to completion (either success or failure)
        let provider_return_uri = ctx
            .complete_mock_bank_redirect_authorization(&redirect_uri, self.mock_bank_action.clone())
            .await
            .unwrap();

        // If we are testing the direct return scenario, submit the return parameters
        if self.redirect_flow == RedirectFlow::DirectReturn {
            let submit_res = ctx
                .client
                .payments
                .submit_provider_return_parameters(
                    &SubmitProviderReturnParametersRequestBuilder::default()
                        .query(provider_return_uri.query().unwrap_or("").to_string())
                        .fragment(provider_return_uri.fragment().unwrap_or("").to_string())
                        .build()
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(
                submit_res.resource,
                SubmitProviderReturnParametersResponseResource::Payment {
                    payment_id: res.id.clone()
                }
            );
        } else {
            ctx.submit_provider_return_parameters(
                provider_return_uri.query().unwrap_or(""),
                provider_return_uri.fragment().unwrap_or(""),
            )
            .await
            .unwrap()
        }

        // Wait for the payment to reach a terminal state
        let payment = payment
            .poll_until_terminal_state(
                &ctx.client,
                PollOptions::default().with_retry_policy(
                    ExponentialBackoff::builder()
                        .build_with_total_retry_duration(Duration::from_secs(60)),
                ),
            )
            .await
            .unwrap();

        // Assert that the payment reached the expected state
        match &self.expected_status {
            ScenarioExpectedStatus::ExecutedOrSettled => {
                assert!(matches!(
                    payment.status,
                    PaymentStatus::Executed { .. } | PaymentStatus::Settled { .. }
                ));
            }
            ScenarioExpectedStatus::Failed {
                failure_stage: expected_failure_stage,
                failure_reason: expected_failure_reason,
            } => {
                assert!(matches!(
                    payment.status,
                    PaymentStatus::Failed {
                        ref failure_stage,
                        ref failure_reason,
                        ..
                    } if failure_stage == expected_failure_stage && failure_reason == expected_failure_reason
                ));
            }
        }
    }
}

// Test all possible combinations of authorization outcomes
#[test_case(
    Currency::Gbp,
    ScenarioBeneficiary::ClosedLoop,
    ScenarioProviderSelection::UserSelected{provider_id: MOCK_PROVIDER_ID_REDIRECT.to_string()},
    MockBankAction::Execute,
    ScenarioExpectedStatus::ExecutedOrSettled,
    RedirectFlow::Classic
    ; "user selected provider successful authorization"
)]
#[test_case(
    Currency::Eur,
    ScenarioBeneficiary::OpenLoop { account_identifier: AccountIdentifier::Iban{ iban: "NL39ABNA8234998285".to_string() } },
    ScenarioProviderSelection::UserSelected{provider_id: MOCK_PROVIDER_ID_ADDITIONAL_INPUTS.to_string()},
    MockBankAction::Execute,
    ScenarioExpectedStatus::ExecutedOrSettled,
    RedirectFlow::Classic
    ; "user selected provider with additional inputs successful authorization"
)]
#[test_case(
    Currency::Gbp,
    ScenarioBeneficiary::ClosedLoop,
    ScenarioProviderSelection::UserSelected{provider_id: MOCK_PROVIDER_ID_REDIRECT.to_string()},
    MockBankAction::RejectAuthorisation,
    ScenarioExpectedStatus::Failed { failure_stage: FailureStage::Authorizing, failure_reason: "authorization_failed" },
    RedirectFlow::Classic
    ; "user selected provider reject authorization"
)]
#[test_case(
    Currency::Gbp,
    ScenarioBeneficiary::ClosedLoop,
    ScenarioProviderSelection::UserSelected{provider_id: MOCK_PROVIDER_ID_REDIRECT.to_string()},
    MockBankAction::RejectExecution,
    ScenarioExpectedStatus::Failed { failure_stage: FailureStage::Authorized, failure_reason: "provider_rejected" },
    RedirectFlow::Classic
    ; "user selected provider reject execution"
)]
#[test_case(
    Currency::Gbp,
    ScenarioBeneficiary::ClosedLoop,
    ScenarioProviderSelection::UserSelected{provider_id: MOCK_PROVIDER_ID_REDIRECT.to_string()},
    MockBankAction::Cancel,
    ScenarioExpectedStatus::Failed { failure_stage: FailureStage::Authorizing, failure_reason: "not_authorized" },
    RedirectFlow::Classic
    ; "user selected provider not authorized"
)]
#[test_case(
    Currency::Gbp,
    ScenarioBeneficiary::ClosedLoop,
    ScenarioProviderSelection::Preselected{provider_id: MOCK_PROVIDER_ID_REDIRECT.to_string(), scheme_id: "faster_payments_service".to_string()},
    MockBankAction::Execute,
    ScenarioExpectedStatus::ExecutedOrSettled,
    RedirectFlow::Classic
    ; "preselected provider successful authorization"
)]
#[test_case(
    Currency::Eur,
    ScenarioBeneficiary::OpenLoop { account_identifier: AccountIdentifier::Iban{ iban: "NL39ABNA8234998285".to_string() } },
    ScenarioProviderSelection::Preselected{provider_id: MOCK_PROVIDER_ID_ADDITIONAL_INPUTS.to_string(), scheme_id:  "sepa_credit_transfer".to_string()},
    MockBankAction::Execute,
    ScenarioExpectedStatus::ExecutedOrSettled,
    RedirectFlow::Classic
    ; "preselected provider with additional inputs successful authorization"
)]
#[test_case(
    Currency::Gbp,
    ScenarioBeneficiary::ClosedLoop,
    ScenarioProviderSelection::Preselected{provider_id: MOCK_PROVIDER_ID_REDIRECT.to_string(), scheme_id: "faster_payments_service".to_string()},
    MockBankAction::RejectAuthorisation,
    ScenarioExpectedStatus::Failed { failure_stage: FailureStage::Authorizing, failure_reason: "authorization_failed" },
    RedirectFlow::Classic
    ; "preselected provider reject authorization"
)]
#[test_case(
    Currency::Gbp,
    ScenarioBeneficiary::ClosedLoop,
    ScenarioProviderSelection::Preselected{provider_id: MOCK_PROVIDER_ID_REDIRECT.to_string(), scheme_id: "faster_payments_service".to_string()},
    MockBankAction::RejectExecution,
    ScenarioExpectedStatus::Failed { failure_stage: FailureStage::Authorized, failure_reason: "provider_rejected" },
    RedirectFlow::Classic
    ; "preselected provider reject execution"
)]
#[test_case(
    Currency::Gbp,
    ScenarioBeneficiary::ClosedLoop,
    ScenarioProviderSelection::Preselected{provider_id: MOCK_PROVIDER_ID_REDIRECT.to_string(), scheme_id: "faster_payments_service".to_string()},
    MockBankAction::Cancel,
    ScenarioExpectedStatus::Failed { failure_stage: FailureStage::Authorizing, failure_reason: "not_authorized" },
    RedirectFlow::Classic
    ; "preselected provider not authorized"
)]
#[test_case(
    Currency::Gbp,
    ScenarioBeneficiary::ClosedLoop,
    ScenarioProviderSelection::UserSelected{provider_id: MOCK_PROVIDER_ID_REDIRECT.to_string()},
    MockBankAction::Execute,
    ScenarioExpectedStatus::ExecutedOrSettled,
    RedirectFlow::DirectReturn
    ; "user selected provider successful authorization direct return"
)]
#[test_case(
    Currency::Gbp,
    ScenarioBeneficiary::ClosedLoop,
    ScenarioProviderSelection::UserSelected{provider_id: MOCK_PROVIDER_ID_REDIRECT.to_string()},
    MockBankAction::RejectAuthorisation,
    ScenarioExpectedStatus::Failed { failure_stage: FailureStage::Authorizing, failure_reason: "authorization_failed" },
    RedirectFlow::DirectReturn
    ; "user selected provider reject authorization direct return"
)]
#[test_case(
    Currency::Gbp,
    ScenarioBeneficiary::ClosedLoop,
    ScenarioProviderSelection::UserSelected{provider_id: MOCK_PROVIDER_ID_REDIRECT.to_string()},
    MockBankAction::RejectExecution,
    ScenarioExpectedStatus::Failed { failure_stage: FailureStage::Authorized, failure_reason: "provider_rejected" },
    RedirectFlow::DirectReturn
    ; "user selected provider reject execution direct return"
)]
#[test_case(
    Currency::Gbp,
    ScenarioBeneficiary::ClosedLoop,
    ScenarioProviderSelection::UserSelected{provider_id: MOCK_PROVIDER_ID_REDIRECT.to_string()},
    MockBankAction::Cancel,
    ScenarioExpectedStatus::Failed { failure_stage: FailureStage::Authorizing, failure_reason: "not_authorized" },
    RedirectFlow::DirectReturn
    ; "user selected provider not authorized direct return"
)]
#[test_case(
    Currency::Gbp,
    ScenarioBeneficiary::ClosedLoop,
    ScenarioProviderSelection::Preselected{provider_id: MOCK_PROVIDER_ID_REDIRECT.to_string(), scheme_id: "faster_payments_service".to_string()},
    MockBankAction::Execute,
    ScenarioExpectedStatus::ExecutedOrSettled,
    RedirectFlow::DirectReturn
    ; "preselected provider successful authorization direct return"
)]
#[test_case(
    Currency::Gbp,
    ScenarioBeneficiary::ClosedLoop,
    ScenarioProviderSelection::Preselected{provider_id: MOCK_PROVIDER_ID_REDIRECT.to_string(), scheme_id: "faster_payments_service".to_string()},
    MockBankAction::RejectAuthorisation,
    ScenarioExpectedStatus::Failed { failure_stage: FailureStage::Authorizing, failure_reason: "authorization_failed" },
    RedirectFlow::DirectReturn
    ; "preselected provider reject authorization direct return"
)]
#[test_case(
    Currency::Gbp,
    ScenarioBeneficiary::ClosedLoop,
    ScenarioProviderSelection::Preselected{provider_id: MOCK_PROVIDER_ID_REDIRECT.to_string(), scheme_id: "faster_payments_service".to_string()},
    MockBankAction::RejectExecution,
    ScenarioExpectedStatus::Failed { failure_stage: FailureStage::Authorized, failure_reason: "provider_rejected" },
    RedirectFlow::DirectReturn
    ; "preselected provider reject execution direct return"
)]
#[test_case(
    Currency::Gbp,
    ScenarioBeneficiary::ClosedLoop,
    ScenarioProviderSelection::Preselected{provider_id: MOCK_PROVIDER_ID_REDIRECT.to_string(), scheme_id: "faster_payments_service".to_string()},
    MockBankAction::Cancel,
    ScenarioExpectedStatus::Failed { failure_stage: FailureStage::Authorizing, failure_reason: "not_authorized" },
    RedirectFlow::DirectReturn
    ; "preselected provider not authorized direct return"
)]
#[test_case(
    Currency::Gbp,
    ScenarioBeneficiary::ClosedLoop,
    ScenarioProviderSelection::Preselected{provider_id: MOCK_PROVIDER_ID_REDIRECT.to_string(), scheme_id: "faster_payments_service".to_string()},
    MockBankAction::Execute,
    ScenarioExpectedStatus::ExecutedOrSettled,
    RedirectFlow::Classic
    ; "closed loop payout"
)]
#[tokio::test]
async fn create_payment_scenarios(
    currency: Currency,
    beneficiary: ScenarioBeneficiary,
    provider_selection: ScenarioProviderSelection,
    mock_bank_action: MockBankAction,
    expected_status: ScenarioExpectedStatus,
    redirect_flow: RedirectFlow,
) {
    CreatePaymentScenario {
        currency,
        beneficiary,
        provider_selection,
        mock_bank_action,
        expected_status,
        redirect_flow,
    }
    .run()
    .await;
}

#[tokio::test]
async fn cancel_payment() {
    let ctx = TestContext::start().await;

    // Create a closed-loop payment
    let payment = helpers::create_closed_loop_payment(&ctx).await.unwrap();

    assert!(matches!(
        payment.status,
        CreatePaymentStatus::AuthorizationRequired { .. }
    ));

    ctx.client.payments.cancel(&payment.id).await.unwrap();

    let payment = ctx
        .client
        .payments
        .get_by_id(&payment.id)
        .await
        .unwrap()
        .unwrap();

    assert!(matches!(
            payment.status,
            PaymentStatus::Failed { failure_reason, failure_stage, .. }
            if failure_reason == *"canceled" && failure_stage == FailureStage::AuthorizationRequired));
}
