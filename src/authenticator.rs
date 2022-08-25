use crate::{
    apis::auth::{AccessToken, AuthenticationResult, Credentials},
    error::Error,
};
use chrono::{Duration, Utc};
use reqwest::Url;
use reqwest_middleware::ClientWithMiddleware;
use tokio::sync::{mpsc, oneshot};

/// Manager for credentials and access tokens.
#[derive(Debug, Clone)]
pub struct Authenticator {
    tx: mpsc::UnboundedSender<oneshot::Sender<Result<AuthenticationResult, Error>>>,
    pub(crate) client_id: String,
}

impl Authenticator {
    /// Starts a new authenticator with the given initial credentials.
    pub fn new(client: ClientWithMiddleware, auth_url: Url, credentials: Credentials) -> Self {
        let client_id = credentials.client_id().to_string();
        let state = AuthenticatorState {
            client,
            auth_url,
            credentials,
            access_token: None,
        };

        // Spawn a long running task which will running forever until the authenticator is dropped
        let (tx, rx) = mpsc::unbounded_channel();
        #[cfg(test)]
        tests::mocked_time::spawn(async move {
            // We need to propagate the mocked time task-local in order to control time in the tests
            process_loop(state, rx).await;
        });
        #[cfg(not(test))]
        tokio::spawn(async move {
            process_loop(state, rx).await;
        });

        Self { tx, client_id }
    }

    /// Returns the current access token used for authentication against the TrueLayer APIs.
    /// If there's no access token available, or the available one has expired, a new one will be requested from the server
    /// using the provided credentials. If the server returns a refresh token, such token will be automatically
    /// used for subsequent refresh attempts.
    ///
    /// Concurrent calls to `get_access_token` are batched into one single request to TrueLayer APIs.
    ///
    /// If the client is already authenticated, this is a no-op.
    pub async fn get_access_token(&self) -> Result<AuthenticationResult, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(tx).unwrap();

        rx.await.unwrap()
    }
}

/// Internal state of the authenticator.
struct AuthenticatorState {
    client: ClientWithMiddleware,
    auth_url: Url,
    credentials: Credentials,
    access_token: Option<AccessToken>,
}

async fn process_loop(
    mut state: AuthenticatorState,
    mut rx: mpsc::UnboundedReceiver<oneshot::Sender<Result<AuthenticationResult, Error>>>,
) {
    // Infinite loop waiting for commands from the main client
    while let Some(reply) = rx.recv().await {
        if reply
            .send(process_get_access_token(&mut state).await)
            .is_err()
        {
            tracing::warn!("Receiver dropped before the reply");
        }
    }
}

#[tracing::instrument(name = "Get Access Token", level = "debug", skip(state))]
async fn process_get_access_token(
    state: &mut AuthenticatorState,
) -> Result<AuthenticationResult, Error> {
    // If we are already authenticated, do nothing
    if let Some(token) = &state.access_token {
        if !should_refresh_token(token) {
            tracing::debug!("Reusing existing access token");
            return Ok(AuthenticationResult {
                access_token: token.clone(),
                refresh_token: state.credentials.refresh_token().cloned(),
            });
        }
    }

    // Post to the auth server with the current credentials.
    // This will use whatever authentication method the user set up.
    let res: RawAuthenticationResponse = state
        .client
        .post(state.auth_url.join("/connect/token").unwrap())
        .json(&state.credentials)
        .send()
        .await?
        .json()
        .await?;

    if res.token_type != "Bearer" {
        return Err(Error::Other(anyhow::anyhow!(
            "Unsupported access token type: {}. This is a bug in the SDK.",
            res.token_type,
        )));
    }

    // Store the access token
    let token = AccessToken {
        token: res.access_token.into(),
        expires_at: Some(now() + Duration::seconds(res.expires_in)),
    };
    state.access_token = Some(token.clone());

    tracing::info!("Got new access token");

    // If a refresh token has been provided, use that for subsequent refreshes of the access token
    if let Some(refresh_token) = &res.refresh_token {
        state.credentials = Credentials::RefreshToken {
            client_id: state.credentials.client_id().to_string(),
            client_secret: state.credentials.client_secret().clone(),
            refresh_token: refresh_token.clone().into(),
        };

        tracing::info!("Switching to refresh token for subsequent authentication requests");
    }

    Ok(AuthenticationResult {
        access_token: token,
        refresh_token: res.refresh_token.map(Token::from),
    })
}

/// Returns `true` if the token is close to expiration (10 minutes before actual expiration)
/// and should be refreshed. If this token does not expire, this function always returns `false`.
fn should_refresh_token(token: &AccessToken) -> bool {
    token.expires_at.map_or(false, |expires_at| {
        now() >= expires_at - Duration::minutes(10)
    })
}

// Select an implementation of `now()` depending on whether we are testing or not
#[cfg(not(test))]
fn now() -> chrono::DateTime<Utc> {
    Utc::now()
}
use crate::apis::auth::Token;
#[cfg(test)]
use tests::mocked_time::now;

/// Successful response of an authentication request.
#[derive(serde::Deserialize)]
struct RawAuthenticationResponse {
    access_token: String,
    expires_in: i64,
    refresh_token: Option<String>,
    token_type: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicU32, Ordering};
    use wiremock::{
        matchers::{body_partial_json, method, path},
        Mock, MockServer, Request, Respond, ResponseTemplate,
    };

    // Internal module to provide mockable time for tests
    #[allow(clippy::declare_interior_mutable_const)]
    pub mod mocked_time {
        use chrono::{DateTime, Utc};
        use std::{
            future::Future,
            sync::{Arc, Mutex},
        };
        use tokio::task::JoinHandle;

        tokio::task_local! {
            static MOCKED_NOW: Arc<Mutex<DateTime<Utc>>>;
        }

        pub fn now() -> DateTime<Utc> {
            MOCKED_NOW.with(|now| *now.lock().unwrap())
        }

        pub fn set_now(new_now: DateTime<Utc>) {
            MOCKED_NOW.with(|now| *now.lock().unwrap() = new_now)
        }

        pub async fn scope<F>(initial_now: DateTime<Utc>, fut: F) -> F::Output
        where
            F: Future,
        {
            MOCKED_NOW
                .scope(Arc::new(Mutex::new(initial_now)), fut)
                .await
        }

        pub fn spawn<F>(fut: F) -> JoinHandle<F::Output>
        where
            F: Future + Send + 'static,
            F::Output: Send + 'static,
        {
            let arc = MOCKED_NOW
                .try_with(|now| now.clone())
                .unwrap_or_else(|_| Arc::new(Mutex::new(Utc::now())));
            tokio::spawn(async move { MOCKED_NOW.scope(arc, fut).await })
        }
    }

    static MOCK_CLIENT_ID: &str = "mock-client-id";
    static MOCK_CLIENT_SECRET: &str = "mock-client-secret";
    static MOCK_ACCESS_TOKEN: &str = "mock-access-token";
    static MOCK_REFRESH_TOKEN: &str = "mock-refresh-token";

    /// Setup a wiremock response that returns a mock access token in the format
    /// `{MOCK_ACCESS_TOKEN}-{count}`, where `count` is the number of requests sent to the mock server.
    fn mock_response(include_refresh_token: bool) -> impl Respond {
        let count = AtomicU32::new(0);
        move |_: &Request| {
            let i = count.fetch_add(1, Ordering::SeqCst);

            ResponseTemplate::new(200).set_body_json(json!({
                "token_type": "Bearer",
                "access_token": format!("{}-{}", MOCK_ACCESS_TOKEN, i),
                "expires_in": 3600,
                "refresh_token": if include_refresh_token { Some(MOCK_REFRESH_TOKEN) } else { None }
            }))
        }
    }

    fn mock_authenticator(auth_url: &str) -> Authenticator {
        let credentials = Credentials::ClientCredentials {
            client_id: MOCK_CLIENT_ID.into(),
            client_secret: MOCK_CLIENT_SECRET.into(),
            scope: "mock".into(),
        };

        Authenticator::new(
            reqwest::Client::new().into(),
            Url::parse(auth_url).unwrap(),
            credentials,
        )
    }

    #[tokio::test]
    async fn access_token_is_reused_until_expired() {
        mocked_time::scope(Utc::now(), async move {
            // Setup mock server
            let mock_server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/connect/token"))
                .and(body_partial_json(json!({
                    "grant_type": "client_credentials",
                    "client_id": MOCK_CLIENT_ID,
                    "client_secret": MOCK_CLIENT_SECRET
                })))
                .respond_with(mock_response(false))
                .expect(2) // Expect exactly two calls
                .mount(&mock_server)
                .await;

            // Setup authenticator
            let authenticator = mock_authenticator(&mock_server.uri());

            // Do two authentication requests
            let auth_result1 = authenticator.get_access_token().await.unwrap();
            let auth_result2 = authenticator.get_access_token().await.unwrap();

            // Assert that we got the same response twice
            assert_eq!(
                auth_result1.access_token.expose_secret(),
                auth_result2.access_token.expose_secret()
            );
            assert_eq!(
                auth_result1.access_token.expires_at(),
                auth_result2.access_token.expires_at()
            );
            assert_eq!(
                auth_result1.refresh_token.as_ref().map(Token::expose_secret),
                auth_result2.refresh_token.as_ref().map(Token::expose_secret)
            );
            assert_eq!(
                auth_result1.access_token.expose_secret(),
                format!("{}-0", MOCK_ACCESS_TOKEN)
            );
            assert!(auth_result1.access_token.expires_at().is_some());
            assert!(auth_result1.refresh_token.is_none());

            // Fast forward time until a moment before token should be refreshed
            mocked_time::set_now(
                auth_result1.access_token.expires_at().unwrap() - Duration::seconds(10 * 60 + 1), /* 10m1s */
            );

            // We still get the same token
            let auth_result3 = authenticator.get_access_token().await.unwrap();
            assert_eq!(
                auth_result1.access_token.expose_secret(),
                auth_result3.access_token.expose_secret()
            );
            assert_eq!(
                auth_result1.access_token.expires_at(),
                auth_result3.access_token.expires_at()
            );

            // Fast forward time until a moment after token expiration
            mocked_time::set_now(
                auth_result1.access_token.expires_at().unwrap() - Duration::minutes(10),
            );

            // We get a new token
            let auth_result4 = authenticator.get_access_token().await.unwrap();
            assert_ne!(
                auth_result1.access_token.expose_secret(),
                auth_result4.access_token.expose_secret()
            );
            assert_eq!(
                auth_result4.access_token.expose_secret(),
                format!("{}-1", MOCK_ACCESS_TOKEN)
            );
            assert!(
                auth_result1.access_token.expires_at().unwrap()
                    < auth_result4.access_token.expires_at().unwrap()
            );
        }).await;
    }

    #[tokio::test]
    async fn refresh_token_is_used_if_provided() {
        mocked_time::scope(Utc::now(), async move {
            // Setup mock server:
            // 1. First mock matches an auth request done with client credentials.
            // 2. Second mock matches an auth request done with a refresh token.
            let mock_server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/connect/token"))
                .and(body_partial_json(json!({
                    "grant_type": "client_credentials",
                    "client_id": MOCK_CLIENT_ID,
                    "client_secret": MOCK_CLIENT_SECRET
                })))
                .respond_with(mock_response(true))
                .expect(1) // Expect exactly one call
                .named("Client credentials mock")
                .mount(&mock_server)
                .await;
            Mock::given(method("POST"))
                .and(path("/connect/token"))
                .and(body_partial_json(json!({
                    "grant_type": "refresh_token",
                    "refresh_token": MOCK_REFRESH_TOKEN,
                    "client_id": MOCK_CLIENT_ID,
                    "client_secret": MOCK_CLIENT_SECRET
                })))
                .respond_with(mock_response(true))
                .expect(1) // Expect exactly one call
                .named("Refresh token mock")
                .mount(&mock_server)
                .await;

            // Setup authenticator
            let authenticator = mock_authenticator(&mock_server.uri());

            // Authenticate the first time. This will use client credentials.
            let res = authenticator.get_access_token().await.unwrap();
            assert_eq!(
                res.refresh_token.unwrap().expose_secret(),
                MOCK_REFRESH_TOKEN
            );

            // Fast forward time to make the token expire
            mocked_time::set_now(res.access_token.expires_at().unwrap());

            // Authenticate again. This will use the refresh token from the previous call.
            let res2 = authenticator.get_access_token().await.unwrap();
            assert_eq!(
                res2.refresh_token.unwrap().expose_secret(),
                MOCK_REFRESH_TOKEN
            );
            assert!(res2.access_token.expires_at.unwrap() > res.access_token.expires_at.unwrap());
        })
        .await;
    }

    #[tokio::test]
    async fn concurrent_requests_are_batched() {
        // Setup mock server
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/connect/token"))
            .respond_with(mock_response(false))
            .expect(1) // Expect exactly one call
            .mount(&mock_server)
            .await;

        // Setup authenticator
        let authenticator = mock_authenticator(&mock_server.uri());

        // Do 100 parallel authentication attempts
        let mut handles = Vec::new();
        for _ in 0..100 {
            let authenticator_clone = authenticator.clone();
            let handle =
                mocked_time::spawn(
                    async move { authenticator_clone.get_access_token().await.unwrap() },
                );
            handles.push(handle);
        }
        let results = futures::future::join_all(handles)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        // Assert that all the attempts yielded the same results
        for res in &results {
            assert_eq!(
                res.access_token.expose_secret(),
                format!("{}-0", MOCK_ACCESS_TOKEN)
            );
        }
    }
}
