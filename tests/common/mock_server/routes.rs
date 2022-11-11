use std::collections::HashMap;

use crate::common::mock_server::{
    MockServerConfiguration, MockServerStorage, MOCK_PROVIDER_ID_ADDITIONAL_INPUTS,
    MOCK_PROVIDER_ID_REDIRECT, MOCK_REDIRECT_URI,
};
use actix_web::{web, HttpResponse};
use chrono::offset::Utc;
use serde_json::json;
use truelayer_rust::apis::{
    auth::Credentials,
    merchant_accounts::{
        ListPaymentSourcesRequest, SetupSweepingRequest, SweepingSettings, Transaction,
        TransactionPayinStatus, TransactionType,
    },
    payments::{
        refunds::{CreateRefundRequest, Refund, RefundStatus},
        AccountIdentifier, AdditionalInput, AdditionalInputDisplayText, AdditionalInputFormat,
        AdditionalInputRegex, AuthorizationFlow, AuthorizationFlowActions,
        AuthorizationFlowNextAction, AuthorizationFlowResponseStatus, BankTransferRequest,
        CreatePaymentRequest, CreatePaymentUserRequest, Currency, ExistingUser, FailureStage,
        NewUser, Payment, PaymentMethod, PaymentSource, PaymentStatus, PreselectedRequest,
        Provider, ProviderSelection, ProviderSelectionRequest, StartAuthorizationFlowRequest,
        StartAuthorizationFlowResponse, SubmitFormActionRequest,
        SubmitProviderReturnParametersRequest, SubmitProviderSelectionActionRequest,
        SubsequentAction, User, UserSelectedRequest,
    },
    payouts::{CreatePayoutRequest, Payout, PayoutStatus},
};
use uuid::Uuid;

/// POST /connect/token
pub(super) async fn post_auth(
    configuration: web::Data<MockServerConfiguration>,
    incoming: web::Json<Credentials>,
) -> HttpResponse {
    match incoming.into_inner() {
        Credentials::ClientCredentials {
            client_id,
            client_secret,
            ..
        } if client_id == configuration.client_id
            && client_secret.expose_secret() == configuration.client_secret =>
        {
            HttpResponse::Ok().json(json!({
                "token_type": "Bearer",
                "access_token": configuration.access_token,
                "expires_in": 3600
            }))
        }
        _ => HttpResponse::BadRequest().json(json!({
            "error": "invalid_client"
        })),
    }
}

/// POST /payments
pub(super) async fn create_payment(
    storage: web::Data<MockServerStorage>,
    create_payment_request: web::Json<CreatePaymentRequest>,
) -> HttpResponse {
    let id = Uuid::new_v4().to_string();
    let user = match create_payment_request.user.clone() {
        CreatePaymentUserRequest::NewUser(NewUser { name, email, phone }) => User {
            id: "payment-source-user-id".to_string(),
            name,
            email,
            phone,
        },
        CreatePaymentUserRequest::ExistingUser(ExistingUser { id }) => User {
            id,
            name: None,
            email: None,
            phone: None,
        },
    };

    let payment_method = match create_payment_request.payment_method.clone() {
        truelayer_rust::apis::payments::PaymentMethodRequest::BankTransfer(
            BankTransferRequest {
                provider_selection,
                beneficiary,
            },
        ) => PaymentMethod::BankTransfer {
            provider_selection: match provider_selection {
                ProviderSelectionRequest::UserSelected(UserSelectedRequest {
                    filter,
                    scheme_selection,
                }) => ProviderSelection::UserSelected {
                    filter,
                    scheme_selection,
                    provider_id: None,
                    scheme_id: None,
                },
                ProviderSelectionRequest::Preselected(PreselectedRequest {
                    provider_id,
                    scheme_id,
                    remitter,
                }) => ProviderSelection::Preselected {
                    provider_id,
                    scheme_id,
                    remitter,
                },
            },
            beneficiary,
        },
    };

    storage.write().unwrap().payments.insert(
        id.clone(),
        (
            Payment {
                id: id.clone(),
                amount_in_minor: create_payment_request.amount_in_minor,
                currency: create_payment_request.currency.clone(),
                user: user.clone(),
                payment_method,
                created_at: Utc::now(),
                status: PaymentStatus::AuthorizationRequired,
                metadata: create_payment_request.metadata.clone(),
            },
            HashMap::new(),
        ),
    );

    HttpResponse::Created().json(json!({
        "id": id,
        "resource_token": format!("resource-token-{}", id),
        "user": {
            "id": user.id
        },
        "status": "authorization_required"
    }))
}

/// GET /payments/{id}
pub(super) async fn get_payment_by_id(
    storage: web::Data<MockServerStorage>,
    path: web::Path<String>,
) -> HttpResponse {
    let id = path.into_inner();

    storage.read().unwrap().payments.get(&id).map_or_else(
        || HttpResponse::NotFound().finish(),
        |payment| HttpResponse::Ok().json(payment.clone().0),
    )
}

/// POST /payments/{id}/authorization-flow
pub(super) async fn start_authorization_flow(
    configuration: web::Data<MockServerConfiguration>,
    storage: web::Data<MockServerStorage>,
    path: web::Path<String>,
    _body: web::Json<StartAuthorizationFlowRequest>, // Just for validation of the body
) -> HttpResponse {
    let id = path.into_inner();

    // Extract the payment from its id
    let mut map = storage.write().unwrap();
    let (payment, _) = match map.payments.get_mut(&id) {
        Some(payment) => payment,
        None => return HttpResponse::NotFound().finish(),
    };

    match payment.status {
        PaymentStatus::AuthorizationRequired => {
            // Choose the next action depending on whether the provider has been preselected or not
            let next_action = match payment.payment_method {
                PaymentMethod::BankTransfer {
                    provider_selection:
                        ProviderSelection::Preselected {
                            ref provider_id, ..
                        },
                    ..
                } => {
                    // Bail out if the user preselected an unexpected provider
                    if !configuration
                        .payments_providers
                        .iter()
                        .any(|p| &p.id == provider_id)
                    {
                        return HttpResponse::BadRequest().finish();
                    }
                    AuthorizationFlowNextAction::Consent {
                        subsequent_action_hint: match provider_id.as_str() {
                            MOCK_PROVIDER_ID_REDIRECT => SubsequentAction::Redirect,
                            MOCK_PROVIDER_ID_ADDITIONAL_INPUTS => SubsequentAction::Form,
                            _ => return HttpResponse::BadRequest().finish(),
                        },
                    }
                }
                _ => AuthorizationFlowNextAction::ProviderSelection {
                    providers: configuration
                        .payments_providers
                        .iter()
                        .map(|p| Provider {
                            id: p.id.clone(),
                            display_name: None,
                            icon_uri: None,
                            logo_uri: None,
                            bg_color: None,
                            country_code: None,
                        })
                        .collect(),
                },
            };

            // Move the payment to the Authorizing state
            let authorization_flow = AuthorizationFlow {
                configuration: None,
                actions: Some(AuthorizationFlowActions { next: next_action }),
            };
            payment.status = PaymentStatus::Authorizing {
                authorization_flow: authorization_flow.clone(),
            };

            HttpResponse::Ok().json(StartAuthorizationFlowResponse {
                authorization_flow: Some(authorization_flow),
                status: AuthorizationFlowResponseStatus::Authorizing,
            })
        }
        _ => HttpResponse::BadRequest().finish(),
    }
}

/// POST /payments/{id}/authorization-flow/provider-selection
pub(super) async fn submit_provider_selection(
    storage: web::Data<MockServerStorage>,
    path: web::Path<String>,
    body: web::Json<SubmitProviderSelectionActionRequest>,
) -> HttpResponse {
    let id = path.into_inner();

    // Extract the payment from its id
    let mut map = storage.write().unwrap();
    let (payment, _) = match map.payments.get_mut(&id) {
        Some(payment) => payment,
        None => return HttpResponse::NotFound().finish(),
    };

    // Update provider selection
    if let PaymentMethod::BankTransfer {
        provider_selection:
            ProviderSelection::UserSelected {
                ref mut provider_id,
                ..
            },
        ..
    } = payment.payment_method
    {
        *provider_id = Some(body.provider_id.clone());
    }

    match payment.status {
        PaymentStatus::Authorizing {
            authorization_flow:
                AuthorizationFlow {
                    actions:
                        Some(AuthorizationFlowActions {
                            next: AuthorizationFlowNextAction::ProviderSelection { .. },
                        }),
                    ..
                },
        } => {
            let authorization_flow = AuthorizationFlow {
                configuration: None,
                actions: Some(AuthorizationFlowActions {
                    next: AuthorizationFlowNextAction::Consent {
                        subsequent_action_hint: match body.provider_id.as_str() {
                            MOCK_PROVIDER_ID_REDIRECT => SubsequentAction::Redirect,
                            MOCK_PROVIDER_ID_ADDITIONAL_INPUTS => SubsequentAction::Form,
                            _ => return HttpResponse::BadRequest().finish(),
                        },
                    },
                }),
            };
            payment.status = PaymentStatus::Authorizing {
                authorization_flow: authorization_flow.clone(),
            };

            HttpResponse::Ok().json(StartAuthorizationFlowResponse {
                authorization_flow: Some(authorization_flow),
                status: AuthorizationFlowResponseStatus::Authorizing,
            })
        }
        _ => HttpResponse::BadRequest().finish(),
    }
}

fn create_form_action() -> AuthorizationFlowNextAction {
    AuthorizationFlowNextAction::Form {
        inputs: vec![
            AdditionalInput::Text {
                id: "psu-branch-code".to_string(),
                mandatory: true,
                display_text: AdditionalInputDisplayText {
                    key: "psu-branch-code.display_text".to_string(),
                    default: "Branch code".to_string(),
                },
                format: AdditionalInputFormat::Any,
                sensitive: true,
                min_length: 3,
                max_length: 3,
                regexes: vec![AdditionalInputRegex {
                    regex: r"^\d{3}$".to_string(),
                    message: AdditionalInputDisplayText {
                        key: "psu-branch-code.regex".to_string(),
                        default: "Validation Regex".to_string(),
                    },
                }],
                description: None,
            },
            AdditionalInput::Text {
                id: "psu-account-number".to_string(),
                mandatory: true,
                display_text: AdditionalInputDisplayText {
                    key: "psu-account-number.display_text".to_string(),
                    default: "Account number".to_string(),
                },
                format: AdditionalInputFormat::Any,
                sensitive: true,
                min_length: 3,
                max_length: 3,
                regexes: vec![AdditionalInputRegex {
                    regex: r"^\d{3}$".to_string(),
                    message: AdditionalInputDisplayText {
                        key: "psu-account-number.regex".to_string(),
                        default: "Validation Regex".to_string(),
                    },
                }],
                description: None,
            },
            AdditionalInput::Text {
                id: "psu-sub-account".to_string(),
                mandatory: true,
                display_text: AdditionalInputDisplayText {
                    key: "psu-sub-account.display_text".to_string(),
                    default: "Sub-account".to_string(),
                },
                format: AdditionalInputFormat::Any,
                sensitive: true,
                min_length: 3,
                max_length: 3,
                regexes: vec![AdditionalInputRegex {
                    regex: r"^\d{3}$".to_string(),
                    message: AdditionalInputDisplayText {
                        key: "psu-sub-account.regex".to_string(),
                        default: "Validation Regex".to_string(),
                    },
                }],
                description: None,
            },
        ],
    }
}

/// POST /payments/{id}/authorization-flow/consent
pub(super) async fn submit_consent(
    storage: web::Data<MockServerStorage>,
    path: web::Path<String>,
) -> HttpResponse {
    let id = path.into_inner();

    // Extract the payment from its id
    let mut map = storage.write().unwrap();
    let (payment, _) = match map.payments.get_mut(&id) {
        Some(payment) => payment,
        None => return HttpResponse::NotFound().finish(),
    };

    // Select the next action (redirect/form) based on the selected provider
    let provider_id = match payment.payment_method {
        PaymentMethod::BankTransfer {
            ref provider_selection,
            ..
        } => match provider_selection {
            ProviderSelection::UserSelected { provider_id, .. } => provider_id.as_deref(),
            ProviderSelection::Preselected { provider_id, .. } => Some(provider_id.as_str()),
        },
    };

    let next_action = match provider_id {
        Some(MOCK_PROVIDER_ID_REDIRECT) => AuthorizationFlowNextAction::Redirect {
            uri: format!("{}{}", MOCK_REDIRECT_URI, payment.id),
            metadata: None,
        },
        Some(MOCK_PROVIDER_ID_ADDITIONAL_INPUTS) => create_form_action(),
        _ => return HttpResponse::BadRequest().finish(),
    };

    match payment.status {
        PaymentStatus::Authorizing {
            authorization_flow:
                AuthorizationFlow {
                    actions:
                        Some(AuthorizationFlowActions {
                            next: AuthorizationFlowNextAction::Consent { .. },
                        }),
                    ..
                },
        } => {
            let authorization_flow = AuthorizationFlow {
                configuration: None,
                actions: Some(AuthorizationFlowActions { next: next_action }),
            };
            payment.status = PaymentStatus::Authorizing {
                authorization_flow: authorization_flow.clone(),
            };

            HttpResponse::Ok().json(StartAuthorizationFlowResponse {
                authorization_flow: Some(authorization_flow),
                status: AuthorizationFlowResponseStatus::Authorizing,
            })
        }
        _ => HttpResponse::BadRequest().finish(),
    }
}

/// POST /payments/{id}/authorization-flow/form
pub(super) async fn submit_form(
    storage: web::Data<MockServerStorage>,
    path: web::Path<String>,
    body: web::Json<SubmitFormActionRequest>,
) -> HttpResponse {
    let id = path.into_inner();

    // We are a very simple and humble mock
    if body.inputs.len() != 3 {
        return HttpResponse::BadRequest().finish();
    }

    // Extract the payment from its id
    let mut map = storage.write().unwrap();
    let (payment, _) = match map.payments.get_mut(&id) {
        Some(payment) => payment,
        None => return HttpResponse::NotFound().finish(),
    };

    match payment.status {
        PaymentStatus::Authorizing {
            authorization_flow:
                AuthorizationFlow {
                    actions:
                        Some(AuthorizationFlowActions {
                            next: AuthorizationFlowNextAction::Form { .. },
                        }),
                    ..
                },
        } => {
            // Set next action to redirect
            let authorization_flow = AuthorizationFlow {
                configuration: None,
                actions: Some(AuthorizationFlowActions {
                    next: AuthorizationFlowNextAction::Redirect {
                        uri: format!("{}{}", MOCK_REDIRECT_URI, payment.id),
                        metadata: None,
                    },
                }),
            };
            payment.status = PaymentStatus::Authorizing {
                authorization_flow: authorization_flow.clone(),
            };

            HttpResponse::Ok().json(StartAuthorizationFlowResponse {
                authorization_flow: Some(authorization_flow),
                status: AuthorizationFlowResponseStatus::Authorizing,
            })
        }
        _ => HttpResponse::BadRequest().finish(),
    }
}

/// POST /payments/{id}/actions/cancel
pub(super) async fn cancel_payment(
    storage: web::Data<MockServerStorage>,
    path: web::Path<String>,
) -> HttpResponse {
    let id = path.into_inner();

    // Extract the payment from its id
    let mut map = storage.write().unwrap();
    let (payment, _) = match map.payments.get_mut(&id) {
        Some(payment) => payment,
        None => return HttpResponse::NotFound().finish(),
    };

    payment.status = PaymentStatus::Failed {
        failed_at: Utc::now(),
        failure_stage: FailureStage::AuthorizationRequired,
        failure_reason: "canceled".into(),
        authorization_flow: None,
    };

    HttpResponse::Accepted().finish()
}

/// GET /payments
pub(super) async fn hpp_page() -> HttpResponse {
    // Intentionally empty. We don't need to do anything here.
    HttpResponse::Ok().finish()
}

/// GET /payments-providers/{id}
pub(super) async fn get_payments_provider_by_id(
    configuration: web::Data<MockServerConfiguration>,
    path: web::Path<String>,
) -> HttpResponse {
    let id = path.into_inner();

    let provider = configuration
        .payments_providers
        .iter()
        .find(|m| m.id == *id);

    match provider {
        Some(p) => HttpResponse::Ok().json(p),
        None => HttpResponse::NotFound().finish(),
    }
}

/// POST /payments/{id}/refunds
pub(super) async fn create_refund(
    storage: web::Data<MockServerStorage>,
    path: web::Path<String>,
    request: web::Json<CreateRefundRequest>,
) -> HttpResponse {
    let payment_id = path.into_inner();

    let mut map = storage.write().unwrap();
    let (payment, refunds) = match map.payments.get_mut(&payment_id) {
        Some(payment) => payment,
        None => return HttpResponse::NotFound().finish(),
    };

    let refund_id = Uuid::new_v4().to_string();
    refunds.insert(
        refund_id.clone(),
        Refund {
            id: refund_id.clone(),
            amount_in_minor: request.amount_in_minor.unwrap_or(payment.amount_in_minor),
            currency: payment.currency.clone(),
            reference: request.reference.clone(),
            created_at: Utc::now(),
            metadata: request.metadata.clone(),
            status: RefundStatus::Executed {
                executed_at: Utc::now(),
            },
        },
    );

    HttpResponse::Created().json(json!({
        "id": refund_id,
    }))
}

/// GET /payments/{id}/refunds/{id}
pub(super) async fn get_refund_by_id(
    storage: web::Data<MockServerStorage>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (payment_id, refund_id) = path.into_inner();

    let mut map = storage.write().unwrap();
    let (_, refunds) = match map.payments.get_mut(&payment_id) {
        Some(payment) => payment,
        None => return HttpResponse::NotFound().finish(),
    };

    match refunds.get(&refund_id) {
        Some(refund) => HttpResponse::Ok().json(refund),
        None => HttpResponse::NotFound().finish(),
    }
}

/// GET /payments/{id}/refunds
pub(super) async fn list_refunds(
    storage: web::Data<MockServerStorage>,
    path: web::Path<String>,
) -> HttpResponse {
    let payment_id = path.into_inner();

    let mut map = storage.write().unwrap();
    let (_, refunds) = match map.payments.get_mut(&payment_id) {
        Some(payment) => payment,
        None => return HttpResponse::NotFound().finish(),
    };

    HttpResponse::Ok().json(json!({
        "items": refunds.values().collect::<Vec<_>>()
    }))
}

/// GET /merchant-accounts
pub(super) async fn list_merchant_accounts(
    configuration: web::Data<MockServerConfiguration>,
) -> HttpResponse {
    HttpResponse::Ok().json(json!({
        "items": configuration.merchant_accounts.values().collect::<Vec<_>>()
    }))
}

/// GET /merchant-accounts/{id}
pub(super) async fn get_merchant_account_by_id(
    configuration: web::Data<MockServerConfiguration>,
    id: web::Path<String>,
) -> HttpResponse {
    let merchant_account = configuration
        .merchant_accounts
        .values()
        .find(|m| m.id == *id);

    match merchant_account {
        Some(m) => HttpResponse::Ok().json(m),
        None => HttpResponse::NotFound().finish(),
    }
}

/// GET /merchant-accounts/{id}/sweeping
pub(super) async fn get_merchant_account_sweeping_by_id(
    storage: web::Data<MockServerStorage>,
    id: web::Path<String>,
) -> HttpResponse {
    let sweeping_settings = storage.read().unwrap().sweeping.get(&*id).cloned();

    match sweeping_settings {
        Some(settings) => HttpResponse::Ok().json(settings),
        None => HttpResponse::NotFound().finish(),
    }
}

/// POST /merchant-accounts/{id}/sweeping
pub(super) async fn setup_merchant_account_sweeping(
    configuration: web::Data<MockServerConfiguration>,
    storage: web::Data<MockServerStorage>,
    id: web::Path<String>,
    request: web::Json<SetupSweepingRequest>,
) -> HttpResponse {
    let iban = configuration.sweeping_approved_ibans.get(&*id).cloned();

    match iban {
        Some(iban) => {
            let request = request.into_inner();
            storage.write().unwrap().sweeping.insert(
                id.clone(),
                SweepingSettings {
                    max_amount_in_minor: request.max_amount_in_minor,
                    currency: request.currency,
                    frequency: request.frequency,
                    destination: AccountIdentifier::Iban { iban },
                },
            );
            HttpResponse::Ok().finish()
        }
        None => HttpResponse::NotFound().finish(),
    }
}

/// DELETE /merchant-accounts/{id}/sweeping
pub(super) async fn disable_merchant_account_sweeping(
    storage: web::Data<MockServerStorage>,
    id: web::Path<String>,
) -> HttpResponse {
    let old = storage.write().unwrap().sweeping.remove(&*id);

    match old {
        Some(_) => HttpResponse::Ok().finish(),
        None => HttpResponse::NotFound().finish(),
    }
}

/// GET /merchant-accounts/{id}/transactions
pub(super) async fn list_transactions(
    configuration: web::Data<MockServerConfiguration>,
    id: web::Path<String>,
) -> HttpResponse {
    let merchant_account = configuration
        .merchant_accounts
        .values()
        .find(|m| m.id == *id);

    match merchant_account {
        Some(_) => HttpResponse::Ok().json(json!({
            "items": vec![Transaction {
                id: "transaction-id-1".into(),
                currency: Currency::Gbp,
                amount_in_minor: 100,
                r#type: TransactionType::MerchantAccountPayment {
                    status: TransactionPayinStatus::Settled,
                    settled_at: Utc::now(),
                    payment_source: PaymentSource {
                        id: "payment-source-id".into(),
                        user_id: Some("payment-source-user-id".into()),
                        account_identifiers: vec![AccountIdentifier::SortCodeAccountNumber {
                            sort_code: "sort-code".to_string(),
                            account_number: "account-number".to_string(),
                        }],
                        account_holder_name: Some("Mr. Holder".into()),
                    },
                    payment_id: "payment-id".into(),
                },
            }]
        })),
        None => HttpResponse::NotFound().finish(),
    }
}

/// GET /merchant-accounts/{id}/payment-sources
pub(super) async fn list_payment_sources(
    configuration: web::Data<MockServerConfiguration>,
    id: web::Path<String>,
    query: web::Query<ListPaymentSourcesRequest>,
) -> HttpResponse {
    let merchant_account = configuration
        .merchant_accounts
        .values()
        .find(|m| m.id == *id);

    match (merchant_account, &*query.user_id) {
        (Some(_), "payment-source-user-id") => HttpResponse::Ok().json(json!({
            "items": vec![PaymentSource {
                id: "payment-source-id".into(),
                user_id: Some("payment-source-user-id".into()),
                account_identifiers: vec![AccountIdentifier::SortCodeAccountNumber {
                    sort_code: "sort-code".to_string(),
                    account_number: "account-number".to_string(),
                }],
                account_holder_name: Some("Mr. Holder".into()),
            }]
        })),
        _ => HttpResponse::NotFound().finish(),
    }
}

/// POST /payouts
pub(super) async fn create_payout(
    configuration: web::Data<MockServerConfiguration>,
    storage: web::Data<MockServerStorage>,
    request: web::Json<CreatePayoutRequest>,
) -> HttpResponse {
    if !configuration
        .merchant_accounts
        .values()
        .any(|m| m.id == request.merchant_account_id)
    {
        return HttpResponse::BadRequest().finish();
    }

    let payout_id = Uuid::new_v4().to_string();
    storage.write().unwrap().payouts.insert(
        payout_id.clone(),
        Payout {
            id: payout_id.clone(),
            merchant_account_id: request.merchant_account_id.clone(),
            amount_in_minor: request.amount_in_minor,
            currency: request.currency.clone(),
            beneficiary: request.beneficiary.clone(),
            created_at: Utc::now(),
            status: PayoutStatus::Pending,
        },
    );

    // Automatically make the payout executed after 1 second
    let payout_id_clone = payout_id.clone();
    tokio::spawn(async move {
        let mut guard = storage.write().unwrap();
        guard.payouts.get_mut(&payout_id_clone).unwrap().status = PayoutStatus::Executed {
            executed_at: Utc::now(),
        };
    });

    HttpResponse::Created().json(json!({ "id": payout_id }))
}

/// GET /payouts/{id}
pub(super) async fn get_payout_by_id(
    storage: web::Data<MockServerStorage>,
    id: web::Path<String>,
) -> HttpResponse {
    storage.read().unwrap().payouts.get(&*id).map_or_else(
        || HttpResponse::NotFound().finish(),
        |payment| HttpResponse::Ok().json(payment),
    )
}

/// POST /payments-provider-return
pub(super) async fn submit_provider_return_parameters(
    req: web::Json<SubmitProviderReturnParametersRequest>,
) -> HttpResponse {
    HttpResponse::Ok().json(json!({
        "resource": {
            "type": "payment",
            "payment_id": req.fragment
        }
    }))
}
