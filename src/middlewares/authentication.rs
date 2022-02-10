use crate::authenticator::Authenticator;
use async_trait::async_trait;
use reqwest::{header::HeaderValue, Request, Response};
use reqwest_middleware::{Middleware, Next};
use task_local_extensions::Extensions;

/// Reqwest middleware to inject the access token into outgoing HTTP requests.
/// On the first request, an additional HTTP request will be fired to get a new access token.
pub struct AuthenticationMiddleware {
    pub(crate) authenticator: Authenticator,
}

#[async_trait]
impl Middleware for AuthenticationMiddleware {
    async fn handle(
        &self,
        mut req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> reqwest_middleware::Result<Response> {
        // Request an access token from the authenticator
        let access_token = self.authenticator.get_access_token().await?;

        // Inject the access token as a header
        let mut header_value = HeaderValue::from_str(&format!(
            "Bearer {}",
            access_token.access_token.expose_secret()
        ))
        .map_err(|e| reqwest_middleware::Error::Middleware(e.into()))?;
        header_value.set_sensitive(true);
        req.headers_mut().insert("Authorization", header_value);

        //Run the rest of the middlewares
        next.run(req, extensions).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apis::auth::Credentials;
    use reqwest::Url;
    use reqwest_middleware::ClientBuilder;
    use serde_json::json;
    use wiremock::{
        matchers::{header, method, path},
        Mock, MockServer, ResponseTemplate,
    };

    static MOCK_CLIENT_ID: &str = "mock-client-id";
    static MOCK_CLIENT_SECRET: &str = "mock-client-secret";
    static MOCK_ACCESS_TOKEN: &str = "mock-access-token";

    fn mock_authenticator(auth_url: &str) -> Authenticator {
        let credentials = Credentials::ClientCredentials {
            client_id: MOCK_CLIENT_ID.to_string(),
            client_secret: MOCK_CLIENT_SECRET.to_string().into(),
            scope: "mock".to_string(),
        };

        Authenticator::new(
            reqwest::Client::new().into(),
            Url::parse(auth_url).unwrap(),
            credentials,
        )
    }

    #[tokio::test]
    async fn access_token_is_attached_to_outgoing_request() {
        // Setup mock server
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/connect/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "token_type": "Bearer",
                "access_token": MOCK_ACCESS_TOKEN,
                "expires_in": 3600,
                "refresh_token": null
            })))
            .expect(1) // Expect exactly one call
            .mount(&mock_server)
            .await;
        Mock::given(method("GET"))
            .and(path("/test"))
            .and(header(
                "Authorization",
                format!("Bearer {}", MOCK_ACCESS_TOKEN).as_str(), // Match the expected token
            ))
            .respond_with(ResponseTemplate::new(200))
            .expect(1) // Expect exactly one call
            .mount(&mock_server)
            .await;

        // Setup a new authenticator
        let authenticator = mock_authenticator(&mock_server.uri());

        // Setup a client using the auth middleware
        let client = ClientBuilder::new(reqwest::Client::new())
            .with(AuthenticationMiddleware { authenticator })
            .build();

        // Send a test request
        client
            .get(format!("{}/test", mock_server.uri()))
            .send()
            .await
            .unwrap();

        // Expectations are verified here before the mock server is dropped
    }
}
