mod middlewares;
mod routes;

use crate::common::{mock_server::middlewares::MiddlewareFn, MockBankAction};
use actix_web::{web, App, HttpServer};
use anyhow::Context;
use chrono::Utc;
use reqwest::Url;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use tokio::sync::oneshot;
use truelayer_rust::apis::{
    merchant_accounts::{MerchantAccount, SweepingSettings},
    payments::{
        AccountIdentifier, AuthorizationFlow, AuthorizationFlowActions,
        AuthorizationFlowNextAction, Currency, FailureStage, Payment, PaymentStatus,
    },
};
use uuid::Uuid;

static MOCK_PROVIDER_ID: &str = "mock-payments-gb-redirect";
static MOCK_REDIRECT_URI: &str = "https://mock.redirect.uri/";

#[derive(Clone)]
struct MockServerConfiguration {
    client_id: String,
    client_secret: String,
    signing_key_id: String,
    signing_public_key: Vec<u8>,
    access_token: String,
    merchant_accounts: HashMap<Currency, MerchantAccount>,
    sweeping_approved_ibans: HashMap<String, String>,
}

#[derive(Clone, Default)]
struct MockServerStorageInner {
    payments: HashMap<String, Payment>,
    sweeping: HashMap<String, SweepingSettings>,
}

/// In-memory storage for payments created on the mock server.
type MockServerStorage = Arc<RwLock<MockServerStorageInner>>;

/// Simple mock server for TrueLayer APIs used in local integration tests.
pub struct TrueLayerMockServer {
    url: Url,
    shutdown: Option<oneshot::Sender<()>>,
    configuration: MockServerConfiguration,
    storage: MockServerStorage,
}

impl TrueLayerMockServer {
    pub async fn start(
        client_id: &str,
        client_secret: &str,
        signing_key_id: &str,
        signing_public_key: Vec<u8>,
    ) -> Self {
        // Prepare the mock server configuration
        let merchant_account_gbp_id = Uuid::new_v4().to_string();
        let configuration = MockServerConfiguration {
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            signing_key_id: signing_key_id.to_string(),
            signing_public_key,
            access_token: Uuid::new_v4().to_string(),
            merchant_accounts: [
                (
                    Currency::Gbp,
                    MerchantAccount {
                        id: merchant_account_gbp_id.clone(),
                        currency: Currency::Gbp,
                        account_identifiers: vec![AccountIdentifier::SortCodeAccountNumber {
                            sort_code: "123456".to_string(),
                            account_number: "12345678".to_string(),
                        }],
                        available_balance_in_minor: 100,
                        current_balance_in_minor: 200,
                        account_holder_name: "Mr. Holder".to_string(),
                    },
                ),
                (
                    Currency::Eur,
                    MerchantAccount {
                        id: Uuid::new_v4().to_string(),
                        currency: Currency::Eur,
                        account_identifiers: vec![AccountIdentifier::Iban {
                            iban: "some-eu-iban".to_string(),
                        }],
                        available_balance_in_minor: 100,
                        current_balance_in_minor: 200,
                        account_holder_name: "Mr. Holder".to_string(),
                    },
                ),
            ]
            .into_iter()
            .collect(),
            sweeping_approved_ibans: [
                // Random IBANs
                (merchant_account_gbp_id, "some-uk-iban".into()),
            ]
            .into_iter()
            .collect(),
        };
        let configuration_clone = configuration.clone();

        // Setup the in-memory storage
        let storage = MockServerStorage::default();
        let storage_clone = storage.clone();

        // Setup the mock HTTP server and bind it to a random port
        let http_server_factory = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(configuration.clone()))
                .app_data(web::Data::new(storage.clone()))
                // User agent must be validated for each request
                .wrap(MiddlewareFn::new(middlewares::validate_user_agent))
                // Mock routes
                .service(web::resource("/connect/token").route(web::post().to(routes::post_auth)))
                .service(
                    web::resource("/payments")
                        .wrap(MiddlewareFn::new(middlewares::ensure_idempotency_key))
                        .wrap(MiddlewareFn::new(middlewares::validate_signature(
                            configuration.clone(),
                            true,
                        )))
                        .route(web::post().to(routes::create_payment))
                        .route(web::get().to(routes::hpp_page)),
                )
                .service(
                    web::resource("/payments/{id}").route(web::get().to(routes::get_payment_by_id)),
                )
                .service(
                    web::resource("/payments/{id}/authorization-flow")
                        .wrap(MiddlewareFn::new(middlewares::ensure_idempotency_key))
                        .wrap(MiddlewareFn::new(middlewares::validate_signature(
                            configuration.clone(),
                            true,
                        )))
                        .route(web::post().to(routes::start_authorization_flow)),
                )
                .service(
                    web::resource("/payments/{id}/authorization-flow/actions/provider-selection")
                        .wrap(MiddlewareFn::new(middlewares::ensure_idempotency_key))
                        .wrap(MiddlewareFn::new(middlewares::validate_signature(
                            configuration.clone(),
                            true,
                        )))
                        .route(web::post().to(routes::submit_provider_selection)),
                )
                .service(
                    web::resource("/merchant-accounts")
                        .route(web::get().to(routes::list_merchant_accounts)),
                )
                .service(
                    web::resource("/merchant-accounts/{id}")
                        .route(web::get().to(routes::get_merchant_account_by_id)),
                )
                .service(
                    web::resource("/merchant-accounts/{id}/sweeping")
                        .wrap(MiddlewareFn::new(middlewares::ensure_idempotency_key))
                        .wrap(MiddlewareFn::new(middlewares::validate_signature(
                            configuration.clone(),
                            true,
                        )))
                        .route(web::get().to(routes::get_merchant_account_sweeping_by_id))
                        .route(web::post().to(routes::setup_merchant_account_sweeping))
                        .route(web::delete().to(routes::disable_merchant_account_sweeping)),
                )
                .service(
                    web::resource("/merchant-accounts/{id}/transactions")
                        .route(web::get().to(routes::list_transactions)),
                )
                .service(
                    web::resource("/merchant-accounts/{id}/payment-sources")
                        .route(web::get().to(routes::list_payment_sources)),
                )
        })
        .workers(1)
        .bind("127.0.0.1:0")
        .unwrap();

        // Retrieve the address and port the server was bound to
        let addr = http_server_factory.addrs().first().cloned().unwrap();

        // Prepare a oneshot channel to kill the HTTP server when this struct is dropped
        let (shutdown_sender, shutdown_recv) = oneshot::channel();

        // Start the server in another task
        let http_server = http_server_factory.run();
        tokio::spawn(async move {
            tokio::select! {
                _ = http_server => panic!("HTTP server crashed"),
                _ = shutdown_recv => { /* Intentional shutdown */ }
            }
        });

        Self {
            url: Url::parse(&format!("http://{}", addr)).unwrap(),
            shutdown: Some(shutdown_sender),
            configuration: configuration_clone,
            storage: storage_clone,
        }
    }

    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn merchant_account(&self, currency: Currency) -> Option<&MerchantAccount> {
        self.configuration.merchant_accounts.get(&currency)
    }

    pub fn sweeping_iban(&self, merchant_account_id: &str) -> Option<String> {
        self.configuration
            .sweeping_approved_ibans
            .get(merchant_account_id)
            .cloned()
    }

    pub async fn complete_mock_bank_redirect_authorization(
        &self,
        redirect_uri: &Url,
        action: MockBankAction,
    ) -> Result<(), anyhow::Error> {
        // Redirect uri is in the form `https://mock.redirect.uri/{payment_id}`
        let payment_id = redirect_uri
            .path_segments()
            .and_then(|mut it| it.next())
            .context("Missing payment id")?;

        let mut storage = self.storage.write().unwrap();
        let payment = storage
            .payments
            .get_mut(payment_id)
            .context("Payment not found")?;

        // Ensure the payment was in Authorizing state waiting for the redirect to complete
        let auth_flow_configuration = match payment.status {
            PaymentStatus::Authorizing {
                authorization_flow:
                    AuthorizationFlow {
                        actions:
                            Some(AuthorizationFlowActions {
                                next: AuthorizationFlowNextAction::Redirect { .. },
                            }),
                        ref configuration,
                    },
                ..
            } => configuration.clone(),
            _ => return Err(anyhow::anyhow!("Invalid payment authorization flow state")),
        };

        let next_auth_flow = AuthorizationFlow {
            actions: None,
            configuration: auth_flow_configuration,
        };

        // Change payment status
        payment.status = match action {
            MockBankAction::Execute => PaymentStatus::Executed {
                executed_at: Utc::now(),
                authorization_flow: Some(next_auth_flow),
            },
            MockBankAction::RejectAuthorisation => PaymentStatus::Failed {
                failed_at: Utc::now(),
                failure_stage: FailureStage::Authorizing,
                failure_reason: "authorization_failed".to_string(),
                authorization_flow: Some(next_auth_flow),
            },
            MockBankAction::RejectExecution => PaymentStatus::Failed {
                failed_at: Utc::now(),
                failure_stage: FailureStage::Authorized,
                failure_reason: "provider_rejected".to_string(),
                authorization_flow: Some(next_auth_flow),
            },
            MockBankAction::Cancel => PaymentStatus::Failed {
                failed_at: Utc::now(),
                failure_stage: FailureStage::Authorizing,
                failure_reason: "canceled".to_string(),
                authorization_flow: Some(next_auth_flow),
            },
        };

        Ok(())
    }
}

impl Drop for TrueLayerMockServer {
    fn drop(&mut self) {
        // Send a shutdown signal to the actix server on drop
        let _ = self.shutdown.take().unwrap().send(());
    }
}
