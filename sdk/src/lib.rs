use auth::{AccessToken, Authentication};
use create_payment::Secrets;
use reqwest::Url;

pub mod auth;
pub mod create_payment;

pub struct Tl {
    access_token: Option<AccessToken>,
    http_client: reqwest::Client,
    secrets: Secrets,
    auth_server: Authentication,
    client: auth::Client,
    environment_uri: Url,
}

pub struct TlBuilder {
    auth_server: Option<Url>,
    http_client: Option<reqwest::Client>,
    secrets: Secrets,
    client: auth::Client,
    environment_uri: Option<Url>,
}

impl TlBuilder {
    pub fn new(secrets: Secrets, client: auth::Client) -> Self {
        Self {
            auth_server: None,
            http_client: None,
            secrets,
            client,
            environment_uri: None,
        }
    }

    pub fn with_auth_server(self, url: Url) -> Self {
        Self {
            auth_server: Some(url),
            ..self
        }
    }

    pub fn with_http_client(self, client: reqwest::Client) -> Self {
        Self {
            http_client: Some(client),
            ..self
        }
    }

    pub fn with_environment_uri(self, url: Url) -> Self {
        Self {
            environment_uri: Some(url),
            ..self
        }
    }

    pub fn build(self) -> Tl {
        Tl {
            access_token: None,
            http_client: self.http_client.unwrap_or_else(reqwest::Client::new),
            secrets: self.secrets,
            client: self.client,
            auth_server: self
                .auth_server
                .map(Authentication::new)
                .unwrap_or_default(),
            environment_uri: self
                .environment_uri
                .unwrap_or_else(|| todo!("what's the default prod uri?")),
        }
    }
}

impl Tl {
    async fn access_token(&mut self) -> Result<&AccessToken, reqwest::Error> {
        // Todo: don't expose reqwest::error directly to user
        match self.access_token {
            Some(ref token) => Ok(token),
            None => {
                let access_token = self
                    .auth_server
                    .get_token(&self.client, &self.http_client)
                    .await?;
                self.access_token = Some(access_token);
                Ok(self.access_token.as_ref().unwrap())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
