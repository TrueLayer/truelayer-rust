use std::str::FromStr;

use reqwest::{multipart::Form, Request};
use secrecy::{ExposeSecret, Secret};
use serde::Deserialize;
use url::Url;

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

pub struct Client {
    /// kid
    id: String,
    secret: Secret<String>,
}

impl Client {
    fn new(id: impl AsRef<str>, secret: impl AsRef<str>) -> Self {
        Self {
            id: id.as_ref().to_string(),
            secret: Secret::new(secret.as_ref().to_string()),
        }
    }
}

#[derive(Deserialize)]
struct AccessToken {
    access_token: String,
    expires_in: u32,
}

impl Authentication {
    fn form(client: Client) -> Form {
        let client_secret = client.secret.expose_secret().clone();
        Form::new()
            .text("scope", "paydirect")
            .text("grant_type", "client_credentials")
            .text("client_id", client.id)
            .text("client_secret", client_secret)
    }

    pub async fn auth(&self, client: Client) -> Result<AccessToken, reqwest::Error> {
        let http_client = reqwest::Client::new();
        let form = Self::form(client);
        http_client
            .post(self.connect_token_endpoint())
            .multipart(form)
            .send()
            .await?
            .json::<AccessToken>()
            .await
    }
}
