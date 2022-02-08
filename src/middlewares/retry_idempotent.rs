use crate::common::IDEMPOTENCY_KEY_HEADER;
use async_trait::async_trait;
use reqwest::{Method, Request, Response};
use reqwest_middleware::{Middleware, Next};
use reqwest_retry::RetryTransientMiddleware;
use retry_policies::{RetryDecision, RetryPolicy};
use std::{
    fmt::{Debug, Formatter},
    sync::Arc,
};
use task_local_extensions::Extensions;

/// Middleware that automatically retries transient failures only on idempotent requests.
///
/// A request is considered idempotent if and only if:
/// - Has an idempotent method (`GET`, `HEAD`, `OPTIONS`, `TRACE`, `PUT` or `DELETE`), or
/// - Has a `POST` or `PATCH` method *and* an `Idempotency-Key` header set.
///
/// For more information regarding idempotent methods, check section 4.2.2 of
/// [RFC 7231](https://datatracker.ietf.org/doc/html/rfc7231#section-4.2.2).
pub struct RetryIdempotentMiddleware {
    inner: RetryTransientMiddleware<DynRetryPolicy>,
}

impl RetryIdempotentMiddleware {
    pub fn new(retry_policy: DynRetryPolicy) -> Self {
        Self {
            inner: RetryTransientMiddleware::new_with_policy(retry_policy),
        }
    }
}

#[async_trait]
impl Middleware for RetryIdempotentMiddleware {
    async fn handle(
        &self,
        req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> reqwest_middleware::Result<Response> {
        let is_idempotent = match *req.method() {
            Method::GET
            | Method::HEAD
            | Method::OPTIONS
            | Method::TRACE
            | Method::PUT
            | Method::DELETE => true,
            Method::POST | Method::PATCH => req
                .headers()
                .get(IDEMPOTENCY_KEY_HEADER)
                .map_or(false, |v| !v.is_empty()),
            _ => false,
        };

        // If the request is idempotent, use the retry middleware, otherwise, do nothing
        if is_idempotent {
            self.inner.handle(req, extensions, next).await
        } else {
            next.run(req, extensions).await
        }
    }
}

/// Wrapper type around a retry policy because `dyn RetryPolicy` does not implement `RetryPolicy`.
#[derive(Clone)]
pub struct DynRetryPolicy(pub Arc<dyn RetryPolicy + Send + Sync + 'static>);

impl RetryPolicy for DynRetryPolicy {
    fn should_retry(&self, n_past_retries: u32) -> RetryDecision {
        self.0.should_retry(n_past_retries)
    }
}

impl Debug for DynRetryPolicy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoxedRetryPolicy").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest_middleware::ClientWithMiddleware;
    use reqwest_retry::policies::ExponentialBackoff;
    use wiremock::{matchers::path, Mock, MockServer, ResponseTemplate};

    async fn mock_client_and_server(expects_retry: bool) -> (ClientWithMiddleware, MockServer) {
        // Configure a mock server that returns 429 Too Many Requests on the first request,
        // and 200 on the second one.
        let mock_server = MockServer::start().await;
        Mock::given(path("/"))
            .respond_with(ResponseTemplate::new(429)) // Too Many Requests
            .expect(1)
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;
        Mock::given(path("/"))
            .respond_with(ResponseTemplate::new(200))
            .expect(if expects_retry { 1 } else { 0 })
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;

        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        let client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new())
            .with(RetryIdempotentMiddleware::new(DynRetryPolicy(Arc::new(
                retry_policy,
            ))))
            .build();

        (client, mock_server)
    }

    #[tokio::test]
    async fn retries_idempotent_methods() {
        for method in [
            Method::GET,
            Method::HEAD,
            Method::OPTIONS,
            Method::TRACE,
            Method::PUT,
            Method::DELETE,
        ] {
            let (client, mock_server) = mock_client_and_server(true).await;

            let res = client
                .request(method, mock_server.uri())
                .send()
                .await
                .unwrap();
            assert!(res.status().is_success());
        }
    }

    #[tokio::test]
    async fn retries_post_patch_with_idempotency_key() {
        for method in [Method::POST, Method::PATCH] {
            let (client, mock_server) = mock_client_and_server(true).await;

            let res = client
                .request(method, mock_server.uri())
                .header(IDEMPOTENCY_KEY_HEADER, "some-idempotency-key")
                .send()
                .await
                .unwrap();
            assert!(res.status().is_success());
        }
    }

    #[tokio::test]
    async fn does_not_retry_post_patch_without_idempotency_key() {
        for method in [Method::POST, Method::PATCH] {
            let (client, mock_server) = mock_client_and_server(false).await;

            let res = client
                .request(method, mock_server.uri())
                .send()
                .await
                .unwrap();
            assert!(res.status().is_client_error());
        }
    }

    #[tokio::test]
    async fn does_not_retry_post_patch_with_empty_idempotency_key() {
        for method in [Method::POST, Method::PATCH] {
            let (client, mock_server) = mock_client_and_server(false).await;

            let res = client
                .request(method, mock_server.uri())
                .header(IDEMPOTENCY_KEY_HEADER, "")
                .send()
                .await
                .unwrap();
            assert!(res.status().is_client_error());
        }
    }
}
