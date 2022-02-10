use crate::common::MockBankAction;
use anyhow::Context;
use truelayer_rust::{apis::auth::Credentials, client::Environment, TrueLayerClient};
use url::Url;

pub struct TestContext {
    pub client: TrueLayerClient,
    pub merchant_account_gbp_id: String,
}

impl TestContext {
    pub async fn start() -> Self {
        // Take the required credentials from the env
        let client_id = std::env::var("ACCEPTANCE_TESTS_CLIENT_ID").unwrap();
        let client_secret = std::env::var("ACCEPTANCE_TESTS_CLIENT_SECRET").unwrap();
        let signing_key_id = std::env::var("ACCEPTANCE_TESTS_SIGNING_KEY_ID").unwrap();
        let signing_private_key = std::env::var("ACCEPTANCE_TESTS_SIGNING_PRIVATE_KEY").unwrap();
        let merchant_account_gbp_id =
            std::env::var("ACCEPTANCE_TESTS_MERCHANT_ACCOUNT_GBP_ID").unwrap();

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
            merchant_account_gbp_id,
        }
    }

    pub fn tl_environment(&self) -> Environment {
        Environment::Sandbox
    }

    pub async fn complete_mock_bank_redirect_authorization(
        &self,
        redirect_uri: &Url,
        action: MockBankAction,
    ) -> Result<(), anyhow::Error> {
        // The redirect uri from mock-bank looks like this:
        // https://pay-mock-connect.truelayer-sandbox.com/login/{simp_id}#token={auth_token}
        let simp_id = redirect_uri
            .path_segments()
            .context("Invalid redirect uri")?
            .nth(1)
            .context("Invalid redirect uri")?;
        let token = &redirect_uri.fragment().context("Invalid redirect uri")?[6..];

        // Make a POST to mock-bank to set the authorization result
        reqwest::Client::new()
            .post(
                redirect_uri
                    .join(&format!(
                        "/api/single-immediate-payments/{}/action",
                        simp_id
                    ))
                    .unwrap(),
            )
            .bearer_auth(token)
            .json(&serde_json::json!({
                "redirect": false,
                "action": action
            }))
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }
}
