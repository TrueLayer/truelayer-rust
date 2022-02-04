mod middlewares;
mod routes;

use crate::common::mock_server::middlewares::MiddlewareFn;
use actix_web::{web, App, HttpServer};
use reqwest::Url;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use tokio::sync::oneshot;
use truelayer_rust::apis::payments::Payment;
use uuid::Uuid;

#[derive(Clone)]
struct MockServerConfiguration {
    client_id: String,
    client_secret: String,
    certificate_id: String,
    certificate_public_key: Vec<u8>,
    access_token: String,
}

type MockServerStorage = Arc<RwLock<HashMap<String, Payment>>>;

/// Simple mock server for TrueLayer APIs used in local integration tests.
pub struct TrueLayerMockServer {
    url: Url,
    shutdown: Option<oneshot::Sender<()>>,
}

impl TrueLayerMockServer {
    pub async fn start(
        client_id: &str,
        client_secret: &str,
        certificate_id: &str,
        certificate_public_key: Vec<u8>,
    ) -> Self {
        let configuration = MockServerConfiguration {
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            certificate_id: certificate_id.to_string(),
            certificate_public_key,
            access_token: Uuid::new_v4().to_string(),
        };

        // Setup the mock HTTP server and bind it to a random port
        let http_server_factory = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(configuration.clone()))
                .app_data(web::Data::new(MockServerStorage::default()))
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
                        .route(web::post().to(routes::create_payment)),
                )
                .service(
                    web::resource("/payments/{id}").route(web::get().to(routes::get_payment_by_id)),
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
        }
    }

    pub fn url(&self) -> &Url {
        &self.url
    }
}

impl Drop for TrueLayerMockServer {
    fn drop(&mut self) {
        // Send a shutdown signal to the actix server on drop
        let _ = self.shutdown.take().unwrap().send(());
    }
}
