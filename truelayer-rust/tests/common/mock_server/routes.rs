use crate::common::mock_server::{MockServerConfiguration, MockServerStorage};
use actix_web::{web, HttpResponse};
use chrono::Utc;
use serde_json::json;
use truelayer_rust::apis::{
    auth::Credentials,
    payments::{CreatePaymentRequest, Payment, PaymentStatus, User},
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
        "payment_token": format!("payment-token-{}", id),
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
