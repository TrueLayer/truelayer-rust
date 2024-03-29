//! Module containing the main TrueLayer API client.
//! This is where the main [`TrueLayerClient`](crate::client::TrueLayerClient) is.

use crate::{
    apis::{
        auth::{AuthApi, Credentials},
        merchant_accounts::MerchantAccountsApi,
        payments::PaymentsApi,
        payments_providers::PaymentsProvidersApi,
        payouts::PayoutsApi,
        TrueLayerClientInner,
    },
    authenticator::Authenticator,
    common::{
        DEFAULT_AUTH_URL, DEFAULT_HOSTED_PAYMENTS_PAGE_URL, DEFAULT_PAYMENTS_URL,
        DEFAULT_SANDBOX_AUTH_URL, DEFAULT_SANDBOX_HOSTED_PAYMENTS_PAGE_URL,
        DEFAULT_SANDBOX_PAYMENTS_URL,
    },
    middlewares::{
        authentication::AuthenticationMiddleware,
        error_handling::ErrorHandlingMiddleware,
        inject_user_agent::InjectUserAgentMiddleware,
        retry_idempotent::{DynRetryPolicy, RetryIdempotentMiddleware},
        signing::SigningMiddleware,
    },
};
use reqwest::Url;
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::{policies::ExponentialBackoff, RetryPolicy};
use reqwest_tracing::TracingMiddleware;
use std::sync::Arc;

/// Client for TrueLayer public APIs.
///
/// ## Authentication
///
/// All TrueLayer endpoints require authentication, and for that reason, a valid set of
/// [`Credentials`] must be provided when building a new client.
///
/// On the first request, the client automatically issues another request to the Auth server
/// to exchange the provided [`Credentials`] for an [`AccessToken`]
/// and caches the received token until it expires. All subsequent requests will reuse the cached
/// token without contacting the Auth server again.
///
/// If needed, you can call the [`get_access_token()`] function to retrieve the current [`AccessToken`],
/// even though that should rarely be necessary.
///
/// ## Idempotency and automatic retries
///
/// In case of a transient failure (e.g., a network error) the client automatically waits and
/// retries the failed request a few times before giving up and returning an error,
/// **only if the original request was idempotent**. Examples of idempotent requests are `GET`s and
/// `DELETE`s (see [RFC 7231] for a complete list).
///
/// In addition to the methods listed in [RFC 7231], the Payments V3 APIs support the usage of
/// [idempotency keys] to also make `POST`s idempotent. The client will transparently attach
/// an auto generated idempotency key to requests against endpoints supporting this feature
/// and thus will also retry them in case of transient failures, without causing unwanted double side-effects.
///
/// To change the retry policy (or to disable automatic retries entirely), use [`with_retry_policy()`]
/// when building a new client.
///
/// ## Request signature
///
/// Some endpoints that have notable side effects (like creating a new payment) require [requests signatures].
/// Signatures are handled automatically by the client if a key is provided at construction time
/// with [`with_signing_key()`].
///
/// [`AccessToken`]: crate::apis::auth::AccessToken
/// [`Credentials`]: crate::apis::auth::Credentials
/// [`get_access_token()`]: crate::apis::auth::AuthApi::get_access_token
/// [`with_retry_policy()`]: crate::client::TrueLayerClientBuilder::with_retry_policy
/// [`with_signing_key()`]: crate::client::TrueLayerClientBuilder::with_signing_key
/// [RFC 7231]: https://datatracker.ietf.org/doc/html/rfc7231#section-4.2.2
/// [idempotency keys]: https://docs.truelayer.com/docs/idempotency
/// [requests signatures]: https://docs.truelayer.com/docs/signing-your-requests
#[derive(Debug, Clone)]
pub struct TrueLayerClient {
    /// Authentication APIs client.
    pub auth: AuthApi,
    /// Payments APIs client.
    pub payments: PaymentsApi,
    /// Payments Providers APIs client.
    pub payments_providers: PaymentsProvidersApi,
    /// Payouts APIs client.
    pub payouts: PayoutsApi,
    /// Merchant Accounts APIs client.
    pub merchant_accounts: MerchantAccountsApi,
}

impl TrueLayerClient {
    /// Builds a new [`TrueLayerClient`](crate::client::TrueLayerClient) with the default configuration.
    pub fn new(credentials: Credentials) -> TrueLayerClient {
        TrueLayerClientBuilder::new(credentials).build()
    }

    /// Returns a new builder to configure a new [`TrueLayerClient`](crate::client::TrueLayerClient).
    pub fn builder(credentials: Credentials) -> TrueLayerClientBuilder {
        TrueLayerClientBuilder::new(credentials)
    }
}

/// Builder for a [`TrueLayerClient`](crate::client::TrueLayerClient).
#[derive(Debug)]
pub struct TrueLayerClientBuilder {
    client: reqwest::Client,
    retry_policy: Option<DynRetryPolicy>,
    environment: Environment,
    credentials: Credentials,
    signing_key: Option<(String, Vec<u8>)>,
}

impl TrueLayerClientBuilder {
    /// Creates a new builder to configure a [`TrueLayerClient`](crate::client::TrueLayerClient).
    pub fn new(credentials: Credentials) -> Self {
        Self {
            client: reqwest::Client::new(),
            retry_policy: Some(DynRetryPolicy(Arc::new(
                ExponentialBackoff::builder().build_with_max_retries(3),
            ))),
            environment: Environment::Live,
            credentials,
            signing_key: None,
        }
    }

    /// Consumes the builder and builds a new [`TrueLayerClient`](crate::client::TrueLayerClient).
    pub fn build(self) -> TrueLayerClient {
        // Build an authenticator
        let authenticator = Authenticator::new(
            build_client_with_middleware(
                self.client.clone(),
                self.retry_policy.clone(),
                None,
                None,
            ),
            self.environment.auth_url(),
            self.credentials,
        );

        // Prepare the middlewares
        let auth_middleware = Some(AuthenticationMiddleware {
            authenticator: authenticator.clone(),
        });
        let signing_middleware = self
            .signing_key
            .map(|(key_id, private_key)| SigningMiddleware {
                key_id,
                private_key,
            });

        // Build the actual TL client
        let inner = Arc::new(TrueLayerClientInner {
            client: build_client_with_middleware(
                self.client,
                self.retry_policy.clone(),
                auth_middleware,
                signing_middleware,
            ),
            environment: self.environment,
            authenticator,
        });

        TrueLayerClient {
            auth: AuthApi::new(inner.clone()),
            payments: PaymentsApi::new(inner.clone()),
            payments_providers: PaymentsProvidersApi::new(inner.clone()),
            payouts: PayoutsApi::new(inner.clone()),
            merchant_accounts: MerchantAccountsApi::new(inner),
        }
    }

    /// Sets a specific reqwest [`Client`](reqwest::Client) to use.
    pub fn with_http_client(mut self, client: reqwest::Client) -> Self {
        self.client = client;
        self
    }

    /// Sets a specific [`RetryPolicy`](retry_policies::RetryPolicy) to use when retrying transient failures.
    ///
    /// To disable automatic retrying of failed requests, use `None`.
    pub fn with_retry_policy(
        mut self,
        retry_policy: impl Into<Option<Arc<dyn RetryPolicy + Send + Sync + 'static>>>,
    ) -> Self {
        self.retry_policy = retry_policy.into().map(DynRetryPolicy);
        self
    }

    /// Configures a signing key for [request signing](https://docs.truelayer.com/docs/signing-your-requests).
    /// Signing is required for some operations like initiating a new payment.
    ///
    /// The private key is expected to be PEM encoded.
    pub fn with_signing_key(mut self, key_id: &str, private_key: Vec<u8>) -> Self {
        self.signing_key = Some((key_id.to_string(), private_key));
        self
    }

    /// Sets the environment to which this client should connect
    pub fn with_environment(mut self, environment: Environment) -> Self {
        self.environment = environment;
        self
    }
}

fn build_client_with_middleware(
    client: reqwest::Client,
    retry_policy: Option<DynRetryPolicy>,
    auth_middleware: Option<AuthenticationMiddleware>,
    signing_middleware: Option<SigningMiddleware>,
) -> ClientWithMiddleware {
    let mut builder = reqwest_middleware::ClientBuilder::new(client)
        .with(InjectUserAgentMiddleware::new())
        .with(TracingMiddleware::default())
        .with(ErrorHandlingMiddleware);

    if let Some(retry_policy) = retry_policy {
        builder = builder.with(RetryIdempotentMiddleware::new(retry_policy));
    }

    if let Some(auth_middleware) = auth_middleware {
        builder = builder.with(auth_middleware);
    }

    if let Some(signing_middleware) = signing_middleware {
        builder = builder.with(signing_middleware);
    }

    builder.build()
}

/// TrueLayer environment to which a [`TrueLayerClient`](crate::client::TrueLayerClient) should connect.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Environment {
    /// TrueLayer Live environment.
    Live,
    /// TrueLayer Sandbox environment.
    Sandbox,
    /// Custom environment. This variant is mainly used for tests.
    Custom {
        auth_url: Url,
        payments_url: Url,
        hpp_url: Url,
    },
}

impl Environment {
    /// Shortcut to build an `Environment::Custom` with all urls set to the given value.
    pub fn from_single_url(url: &Url) -> Environment {
        Environment::Custom {
            auth_url: url.clone(),
            payments_url: url.clone(),
            hpp_url: url.clone(),
        }
    }

    /// Base URL for authentication-related requests.
    pub fn auth_url(&self) -> Url {
        match self {
            Environment::Live => Url::parse(DEFAULT_AUTH_URL).unwrap(),
            Environment::Sandbox => Url::parse(DEFAULT_SANDBOX_AUTH_URL).unwrap(),
            Environment::Custom { auth_url, .. } => auth_url.clone(),
        }
    }

    /// Base URL for payments-related requests.
    pub fn payments_url(&self) -> Url {
        match self {
            Environment::Live => Url::parse(DEFAULT_PAYMENTS_URL).unwrap(),
            Environment::Sandbox => Url::parse(DEFAULT_SANDBOX_PAYMENTS_URL).unwrap(),
            Environment::Custom { payments_url, .. } => payments_url.clone(),
        }
    }

    /// Base URL for the Hosted Payments Page.
    pub fn hpp_url(&self) -> Url {
        match self {
            Environment::Live => Url::parse(DEFAULT_HOSTED_PAYMENTS_PAGE_URL).unwrap(),
            Environment::Sandbox => Url::parse(DEFAULT_SANDBOX_HOSTED_PAYMENTS_PAGE_URL).unwrap(),
            Environment::Custom { hpp_url, .. } => hpp_url.clone(),
        }
    }
}
