use crate::common::mock_server::TrueLayerMockServer;
use openssl::{
    ec::{EcGroup, EcKey},
    nid::Nid,
};
use reqwest::Url;
use truelayer_rust::{apis::auth::Credentials, TrueLayerClient};
use uuid::Uuid;

pub struct TestContext {
    pub client: TrueLayerClient,
    mock_server: TrueLayerMockServer,
}

impl TestContext {
    pub async fn start() -> Self {
        // Generate a new set of random credentials for this specific test
        let client_id = Uuid::new_v4().to_string();
        let client_secret = Uuid::new_v4().to_string();
        let certificate_id = Uuid::new_v4().to_string();
        let certificate_private_key =
            EcKey::generate(&EcGroup::from_curve_name(Nid::SECP521R1).unwrap()).unwrap();

        // Setup a new mock server
        let mock_server = TrueLayerMockServer::start(
            &client_id,
            &client_secret,
            &certificate_id,
            certificate_private_key.public_key_to_pem().unwrap(),
        )
        .await;

        // Configure a new TrueLayerClient to point to the mock server
        let client = TrueLayerClient::builder(Credentials::ClientCredentials {
            client_id: client_id.clone(),
            client_secret: client_secret.clone(),
            scope: "payments paydirect".to_string(),
        })
        .with_certificate(
            &certificate_id,
            certificate_private_key.private_key_to_pem().unwrap(),
        )
        .with_retry_policy(None) // Disable retries against the mock server
        .with_auth_url(mock_server.url().clone())
        .with_payments_url(mock_server.url().clone())
        .with_hosted_payments_page_url(mock_server.url().clone())
        .build();

        Self {
            client,
            mock_server,
        }
    }

    pub fn mock_server_url(&self) -> &Url {
        self.mock_server.url()
    }
}
