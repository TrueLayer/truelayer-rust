use secrecy::{ExposeSecret, Secret};
use serde::Deserialize;
use url::Url;
use uuid::Uuid;

#[derive(Debug)]
pub struct Authentication {
    auth_server: Url,
}

impl Default for Authentication {
    fn default() -> Self {
        Self {
            auth_server: Url::parse("https://auth.truelayer.com/").unwrap(),
        }
    }
}

impl Authentication {
    pub fn new(auth_server: impl AsRef<str>) -> Result<Self, url::ParseError> {
        Ok(Self {
            auth_server: Url::parse(auth_server.as_ref())?,
        })
    }

    fn connect_token_endpoint(&self) -> Url {
        self.auth_server.join("connect/token").unwrap()
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
pub struct AccessToken {
    access_token: String,
    expires_in: u32,
}

impl Authentication {
    fn form(client: Client) -> serde_json::Value {
        let client_secret = client.secret.expose_secret().clone();

        serde_json::json!({
            "scope": "paydirect",
            "grant_type": "client_credentials",
            "client_id": client.id,
            "client_secret": client_secret,
        })
    }

    pub async fn auth(&self, client: Client) -> Result<AccessToken, reqwest::Error> {
        let http_client = reqwest::Client::new();
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