use secrecy::{ExposeSecret, Secret};
use serde::Deserialize;
use tracing::info;
use url::Url;
use uuid::Uuid;

#[derive(Debug)]
pub(crate) struct Authentication {
    auth_server: Url,
    access_token: Option<AccessToken>,

}

impl Default for Authentication {
    fn default() -> Self {
        Self {
            auth_server: Url::parse("https://auth.truelayer.com/").unwrap(),
            access_token: None,
        }
    }
}

impl Authentication {
    pub(crate) fn new(auth_server: Url) -> Self {
        Self {
            auth_server,
            access_token: None,
        }
    }

    fn connect_token_endpoint(&self) -> Url {
        self.auth_server.join("connect/token").unwrap()
    }

    pub(crate) async fn access_token(
        &mut self,
        client: &Client,
        http_client: &reqwest::Client,
    ) -> Result<&AccessToken, reqwest::Error> {
        // Todo: don't expose reqwest::error directly to user
        match self.access_token {
            Some(ref token) => {
                info!("using existing access token");
                Ok(token)
            }
            None => {
                let access_token = self
                    .get_token(client, http_client)
                    .await?;
                info!("retrieved new access token");
                self.access_token = Some(access_token);
                Ok(self.access_token.as_ref().unwrap())
            }
        }
    }
}

#[derive(Debug)]
pub struct Client {
    /// client id
    id: String,
    /// This secret is a [`Uuid`], but the [`Uuid`] type is not compatible with [`Secret`].
    /// So we treat it as a [`String`]
    secret: Secret<String>,
}

impl Client {
    pub fn new(id: String, secret: Uuid) -> Self {
        Self {
            id,
            secret: Secret::new(secret.to_string()),
        }
    }
}

#[derive(Deserialize, Debug)]
pub(crate) struct AccessToken {
    pub access_token: String,
    expires_in: u32,
}

impl Authentication {
    fn form(client: &Client) -> serde_json::Value {
        let client_secret = client.secret.expose_secret().clone();

        serde_json::json!({
            "scope": "paydirect",
            "grant_type": "client_credentials",
            "client_id": client.id,
            "client_secret": client_secret,
        })
    }

    /// Get access token from auth server
    pub(crate) async fn get_token(
        &self,
        client: &Client,
        http_client: &reqwest::Client,
    ) -> Result<AccessToken, reqwest::Error> {
        let form = Self::form(client);
        http_client
            .post(self.connect_token_endpoint())
            .form(&form)
            .send()
            .await?
            .json::<AccessToken>()
            .await
    }
}
