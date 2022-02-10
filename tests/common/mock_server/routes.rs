use crate::common::mock_server::{
    MockServerConfiguration, MockServerStorage, MOCK_PROVIDER_ID, MOCK_REDIRECT_URI,
};
use actix_web::{web, HttpResponse};
use chrono::Utc;
use serde_json::json;
use truelayer_rust::apis::{
    auth::Credentials,
    payments::{
        AuthorizationFlow, AuthorizationFlowActions, AuthorizationFlowNextAction,
        AuthorizationFlowResponseStatus, CreatePaymentRequest, Payment, PaymentMethod,
        PaymentStatus, Provider, ProviderSelection, StartAuthorizationFlowRequest,
        StartAuthorizationFlowResponse, SubmitProviderSelectionActionRequest, User,
    },
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
            && client_secret == configuration.client_secret =>
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
    let user_id = create_payment_request
        .user
        .id
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    storage.write().unwrap().insert(
        id.clone(),
        Payment {
            id: id.clone(),
            amount_in_minor: create_payment_request.amount_in_minor,
            currency: create_payment_request.currency.clone(),
            user: User {
                id: Some(user_id.clone()),
                ..create_payment_request.user.clone()
            },
            payment_method: create_payment_request.payment_method.clone(),
            created_at: Utc::now(),
            status: PaymentStatus::AuthorizationRequired,
        },
    );

    HttpResponse::Created().json(json!({
        "id": id,
        "resource_token": format!("resource-token-{}", id),
        "user": {
            "id": user_id
        }
    }))
}

/// GET /payments/{id}
pub(super) async fn get_payment_by_id(
    storage: web::Data<MockServerStorage>,
    path: web::Path<String>,
) -> HttpResponse {
    let id = path.into_inner();

    storage.read().unwrap().get(&id).map_or_else(
        || HttpResponse::NotFound().finish(),
        |payment| HttpResponse::Ok().json(payment),
    )
}

/// POST /payments/{id}/authorization-flow
pub(super) async fn start_authorization_flow(
    storage: web::Data<MockServerStorage>,
    path: web::Path<String>,
    _body: web::Json<StartAuthorizationFlowRequest>, // Just for validation of the body
) -> HttpResponse {
    let id = path.into_inner();

    // Extract the payment from its id
    let mut map = storage.write().unwrap();
    let payment = match map.get_mut(&id) {
        Some(payment) => payment,
        None => return HttpResponse::NotFound().finish(),
    };

    // Check if the payment was created with a preselected provider
    let is_provider_preselected = match payment.payment_method {
        PaymentMethod::BankTransfer {
            provider_selection:
                ProviderSelection::Preselected {
                    ref provider_id, ..
                },
            ..
        } => {
            // Bail out if the user preselected an unexpected provider
            if provider_id != MOCK_PROVIDER_ID {
                return HttpResponse::BadRequest().finish();
            }

            true
        }
        _ => false,
    };

    match payment.status {
        PaymentStatus::AuthorizationRequired => {
            // Choose the next action depending on whether the provider has already been preselected or not
            let next_action = if is_provider_preselected {
                AuthorizationFlowNextAction::Redirect {
                    uri: format!("{}{}", MOCK_REDIRECT_URI, payment.id),
                    metadata: None,
                }
            } else {
                AuthorizationFlowNextAction::ProviderSelection {
                    providers: vec![Provider {
                        provider_id: MOCK_PROVIDER_ID.to_string(),
                        display_name: None,
                        icon_uri: None,
                        logo_uri: None,
                        bg_color: None,
                        country_code: None,
                    }],
                }
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

    // We are a very simple and humble mock
    if body.provider_id != MOCK_PROVIDER_ID {
        return HttpResponse::BadRequest().finish();
    }

    // Extract the payment from its id
    let mut map = storage.write().unwrap();
    let payment = match map.get_mut(&id) {
        Some(payment) => payment,
        None => return HttpResponse::NotFound().finish(),
    };

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

/// GET /payments
pub(super) async fn hpp_page() -> HttpResponse {
    // Intentionally empty. We don't need to do anything here.
    HttpResponse::Ok().finish()
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
