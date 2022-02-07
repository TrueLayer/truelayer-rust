use truelayer_rust::{apis::auth::Credentials, client::Environment, TrueLayerClient};

pub struct TestContext {
    pub client: TrueLayerClient,
    pub merchant_account_id: String,
}

impl TestContext {
    pub async fn start() -> Self {
        // Take the required credentials from the env
        let client_id = std::env::var("ACCEPTANCE_TESTS_CLIENT_ID").unwrap();
        let client_secret = std::env::var("ACCEPTANCE_TESTS_CLIENT_SECRET").unwrap();
        let signing_key_id = std::env::var("ACCEPTANCE_TESTS_SIGNING_KEY_ID").unwrap();
        let signing_private_key = std::env::var("ACCEPTANCE_TESTS_SIGNING_PRIVATE_KEY").unwrap();
        let merchant_account_id = std::env::var("ACCEPTANCE_TESTS_MERCHANT_ACCOUNT_ID").unwrap();

        // Configure a new TrueLayerClient to point to Sandbox
        let client = TrueLayerClient::builder(Credentials::ClientCredentials {
            client_id,
            client_secret,
            scope: "payments paydirect".to_string(),
        })
        .with_signing_key(&signing_key_id, signing_private_key.into_bytes())
        .with_environment(Environment::Sandbox)
        .build();

        Self {
            client,
            merchant_account_id,
        }
    }

    pub fn tl_environment(&self) -> Environment {
        Environment::Sandbox
    }
}
