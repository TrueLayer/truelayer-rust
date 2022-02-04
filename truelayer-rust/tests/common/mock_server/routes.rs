use crate::common::mock_server::MockServerConfiguration;
use actix_web::{web, HttpResponse};
use serde_json::json;
use truelayer_rust::apis::auth::Credentials;
use truelayer_rust::apis::payments::CreatePaymentRequest;

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
    create_payment_request: web::Json<CreatePaymentRequest>,
) -> HttpResponse {
    HttpResponse::InternalServerError().finish()
}

/// GET /payments/{id}
pub(super) async fn get_payment_by_id(id: String) -> HttpResponse {
    HttpResponse::InternalServerError().finish()
}
