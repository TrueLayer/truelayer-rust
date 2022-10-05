use crate::common::{mock_server::TrueLayerMockServer, MockBankAction};
use openssl::{
    ec::{EcGroup, EcKey},
    nid::Nid,
};
use truelayer_rust::{
    apis::{auth::Credentials, payments::Currency},
    client::Environment,
    TrueLayerClient,
};
use url::Url;
use uuid::Uuid;

pub struct TestContext {
    pub client: TrueLayerClient,
    pub merchant_account_gbp_id: String,
    pub merchant_account_gbp_sweeping_iban: String,
    mock_server: TrueLayerMockServer,
}

impl TestContext {
    pub async fn start() -> Self {
        // Generate a new set of random credentials for this specific test
        let client_id = Uuid::new_v4().to_string();
        let client_secret = Uuid::new_v4().to_string();
        let signing_key_id = Uuid::new_v4().to_string();
        let signing_private_key =
            EcKey::generate(&EcGroup::from_curve_name(Nid::SECP521R1).unwrap()).unwrap();

        // Setup a new mock server
        let mock_server = TrueLayerMockServer::start(
            &client_id,
            &client_secret,
            &signing_key_id,
            signing_private_key.public_key_to_pem().unwrap(),
        )
        .await;

        // Configure a new TrueLayerClient to point to the mock server
        let client = TrueLayerClient::builder(Credentials::ClientCredentials {
            client_id: client_id.clone(),
            client_secret: client_secret.clone().into(),
            scope: "payments paydirect".to_string(),
        })
        .with_signing_key(
            &signing_key_id,
            signing_private_key.private_key_to_pem().unwrap(),
        )
        .with_retry_policy(None) // Disable retries against the mock server
        .with_environment(Environment::from_single_url(mock_server.url()))
        .build();

        let merchant_account_gbp_id = mock_server
            .merchant_account(Currency::Gbp)
            .map(|m| m.id.clone())
            .unwrap();

        Self {
            client,
            merchant_account_gbp_sweeping_iban: mock_server
                .sweeping_iban(&merchant_account_gbp_id)
                .unwrap(),
            merchant_account_gbp_id,
            mock_server,
        }
    }

    pub fn tl_environment(&self) -> Environment {
        Environment::from_single_url(self.mock_server.url())
    }

    pub async fn complete_mock_bank_redirect_authorization(
        &self,
        redirect_uri: &Url,
        action: MockBankAction,
    ) -> Result<Url, anyhow::Error> {
        self.mock_server
            .complete_mock_bank_redirect_authorization(redirect_uri, action)
            .await
    }

    pub async fn submit_provider_return_parameters(
        &self,
        _query: String,
        _fragment: String,
    ) -> Result<(), anyhow::Error> {
        // This is only necessary for acceptance tests to work correctly.
        // This work is usually done by TrueLayer's SPA upon redirect from the provider.
        Ok(())
    }
}
