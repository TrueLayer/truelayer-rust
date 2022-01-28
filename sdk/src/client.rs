//! Module containing the main TrueLayer API client.

use crate::{
    apis::{
        auth::{AuthApi, Credentials},
        payments::PaymentsApi,
        TrueLayerClientInner,
    },
    authenticator::Authenticator,
    middlewares::{
        authentication::AuthenticationMiddleware,
        error_handling::ErrorHandlingMiddleware,
        retry_idempotent::{BoxedRetryPolicy, RetryIdempotentMiddleware},
        signing::SigningMiddleware,
    },
};
use reqwest::Url;
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::{policies::ExponentialBackoff, RetryPolicy};
use reqwest_tracing::TracingMiddleware;
use std::sync::Arc;

static DEFAULT_AUTH_URL: &str = "https://auth.truelayer.com";
static DEFAULT_PAYMENTS_URL: &str = "https://test-pay-api.truelayer.com";
static DEFAULT_HOSTED_PAYMENTS_PAGE_URL: &str = "https://payment.truelayer.com";
static DEFAULT_SANDBOX_AUTH_URL: &str = "https://auth.truelayer-sandbox.com";
static DEFAULT_SANDBOX_PAYMENTS_URL: &str = "https://test-pay-api.truelayer-sandbox.com";
static DEFAULT_SANDBOX_HOSTED_PAYMENTS_PAGE_URL: &str = "https://payment.truelayer-sandbox.com";

/// Client for TrueLayer public APIs.
///
/// TODO: Describe idempotency key and automatic retries.
#[derive(Debug, Clone)]
pub struct TrueLayerClient {
    /// Authentication APIs client.
    pub auth: AuthApi,
    /// Payments APIs client.
    pub payments: PaymentsApi,
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
    retry_policy: Option<BoxedRetryPolicy>,
    auth_url: Url,
    payments_url: Url,
    hpp_url: Url,
    credentials: Credentials,
    certificate: Option<(String, Vec<u8>)>,
}

impl TrueLayerClientBuilder {
    /// Creates a new builder to configure a [`TrueLayerClient`](crate::client::TrueLayerClient).
    pub fn new(credentials: Credentials) -> Self {
        Self {
            client: reqwest::Client::new(),
            retry_policy: Some(BoxedRetryPolicy(Arc::new(
                ExponentialBackoff::builder().build_with_max_retries(3),
            ))),
            auth_url: Url::parse(DEFAULT_AUTH_URL).unwrap(),
            payments_url: Url::parse(DEFAULT_PAYMENTS_URL).unwrap(),
            hpp_url: Url::parse(DEFAULT_HOSTED_PAYMENTS_PAGE_URL).unwrap(),
            credentials,
            certificate: None,
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
            self.auth_url,
            self.credentials,
        );

        // Prepare the middlewares
        let auth_middleware = Some(AuthenticationMiddleware {
            authenticator: authenticator.clone(),
        });
        let signing_middleware = self.certificate.map(
            |(certificate_id, certificate_private_key)| SigningMiddleware {
                certificate_id,
                certificate_private_key,
            },
        );

        // Build the actual TL client
        let inner = Arc::new(TrueLayerClientInner {
            client: build_client_with_middleware(
                self.client,
                self.retry_policy.clone(),
                auth_middleware,
                signing_middleware,
            ),
            authenticator,
            payments_url: self.payments_url,
            hpp_url: self.hpp_url,
        });

        TrueLayerClient {
            auth: AuthApi::new(inner.clone()),
            payments: PaymentsApi::new(inner),
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
        self.retry_policy = retry_policy.into().map(BoxedRetryPolicy);
        self
    }

    /// Configures a certificate for [request signing](https://docs.truelayer.com/docs/paydirect-sign-requests).
    /// Signing is required for some operations like initiating a new payment.
    pub fn with_certificate(mut self, certificate_id: &str, private_key_pem: Vec<u8>) -> Self {
        self.certificate = Some((certificate_id.to_string(), private_key_pem));
        self
    }

    /// Sets the base URL for authentication-related requests.
    ///
    /// Defaults to: `https://auth.truelayer.com`
    pub fn with_auth_url(mut self, auth_url: Url) -> Self {
        self.auth_url = auth_url;
        self
    }

    /// Sets the base URL for payments-related requests.
    ///
    /// Defaults to: `https://test-pay-api.truelayer.com`
    pub fn with_payments_url(mut self, payments_url: Url) -> Self {
        self.payments_url = payments_url;
        self
    }

    /// Sets the base URL for any generated Hosted Payments Page link.
    ///
    /// Defaults to: `https://payment.truelayer.com`
    pub fn with_hosted_payments_page_url(mut self, hpp_url: Url) -> Self {
        self.hpp_url = hpp_url;
        self
    }

    /// Sets all the base URL to their sandbox values.
    pub fn with_sandbox_urls(self) -> Self {
        self.with_auth_url(Url::parse(DEFAULT_SANDBOX_AUTH_URL).unwrap())
            .with_payments_url(Url::parse(DEFAULT_SANDBOX_PAYMENTS_URL).unwrap())
            .with_hosted_payments_page_url(
                Url::parse(DEFAULT_SANDBOX_HOSTED_PAYMENTS_PAGE_URL).unwrap(),
            )
    }
}

fn build_client_with_middleware(
    client: reqwest::Client,
    retry_policy: Option<BoxedRetryPolicy>,
    auth_middleware: Option<AuthenticationMiddleware>,
    signing_middleware: Option<SigningMiddleware>,
) -> ClientWithMiddleware {
    let mut builder = reqwest_middleware::ClientBuilder::new(client)
        .with(TracingMiddleware)
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
