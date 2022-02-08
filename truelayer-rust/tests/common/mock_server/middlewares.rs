use crate::common::mock_server::MockServerConfiguration;
use actix_web::{
    body::BoxBody,
    dev::{Payload, Service, ServiceRequest, ServiceResponse, Transform},
    error::PayloadError,
    http::Method,
    web::{Bytes, BytesMut},
    Error, HttpMessage, HttpResponse,
};
use anyhow::anyhow;
use futures::{
    future::{LocalBoxFuture, Ready},
    FutureExt, StreamExt, TryFutureExt, TryStreamExt,
};
use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll},
};

/// Middleware to check that all the requests contain the right user agent header
pub(super) async fn validate_user_agent(req: &mut ServiceRequest) -> Result<(), anyhow::Error> {
    // Check that the User-Agent header is present
    anyhow::ensure!(
        req.headers()
            .get("User-Agent")
            .map(|v| v.to_str())
            .transpose()?
            == Some(concat!("truelayer-rust/", env!("CARGO_PKG_VERSION"))),
        "Invalid User-Agent"
    );

    Ok(())
}

/// Ensures that the incoming request has an idempotency key set
pub(super) async fn ensure_idempotency_key(req: &mut ServiceRequest) -> Result<(), anyhow::Error> {
    // Skip this middleware for GETs
    if req.method() == Method::GET {
        return Ok(());
    }

    anyhow::ensure!(
        req.headers()
            .get("Idempotency-Key")
            .map(|v| v.to_str())
            .transpose()?
            .map_or(false, |v| !v.is_empty()),
        "Invalid or missing Idempotency Key"
    );

    Ok(())
}

/// Validates a full request signature
pub(super) fn validate_signature(
    configuration: MockServerConfiguration,
    require_idempotency_key: bool,
) -> impl Fn(&mut ServiceRequest) -> LocalBoxFuture<'_, Result<(), anyhow::Error>> {
    let configuration = Arc::new(configuration);

    move |req: &mut ServiceRequest| {
        let configuration = configuration.clone();

        Box::pin(async move {
            // Skip this middleware for GETs
            if req.method() == Method::GET {
                return Ok(());
            }

            // Buffer all the body in memory
            let body = req
                .take_payload()
                .try_fold(BytesMut::new(), |mut body, chunk| async move {
                    body.extend_from_slice(&chunk);
                    Ok::<_, PayloadError>(body)
                })
                .map_err(anyhow::Error::from)
                .await?;

            let mut verifier =
                truelayer_signing::verify_with_pem(configuration.signing_public_key.as_slice())
                    .method(req.method().as_str())
                    .path(req.path());

            if require_idempotency_key {
                let idempotency_key = req
                    .headers()
                    .get("Idempotency-Key")
                    .map(|v| v.as_bytes())
                    .ok_or_else(|| anyhow!("Missing required idempotency key"))?;
                verifier = verifier
                    .require_header("Idempotency-Key")
                    .header("Idempotency-Key", idempotency_key);
            }

            // Validate the signature
            let signature = req
                .headers()
                .get("Tl-Signature")
                .map(|v| v.to_str())
                .transpose()?
                .ok_or_else(|| anyhow!("Missing required signature"))?;
            if truelayer_signing::extract_jws_header(signature)?.kid != configuration.signing_key_id
            {
                return Err(anyhow!("Invalid key id"));
            }
            verifier.body(&body).verify(signature)?;

            // Put the body back into the request so that it can be consumed by other middlewares
            req.set_payload(Payload::Stream {
                payload: futures::stream::once(
                    async move { Ok::<_, PayloadError>(Bytes::from(body)) },
                )
                .boxed(),
            });

            Ok(())
        })
    }
}

/// Helper trait used to circumvent a limitation of Rust's Higher Ranked Trait Bounds
/// in the implementation of `MiddlewareFnWrapper::call`.
/// For more info see: https://users.rust-lang.org/t/higher-rank-trait-bounds-use-bound-lifetime-in-another-generic/45121
pub(super) trait CallableAsyncFn<'r> {
    type Output: Future<Output = Result<(), anyhow::Error>> + 'r;

    fn call(&self, req: &'r mut ServiceRequest) -> Self::Output;
}

impl<'r, F, R> CallableAsyncFn<'r> for F
where
    F: Fn(&'r mut ServiceRequest) -> R,
    R: Future<Output = Result<(), anyhow::Error>> + 'r,
{
    type Output = R;

    fn call(&self, req: &'r mut ServiceRequest) -> Self::Output {
        self(req)
    }
}

/// Wrapper around a function to act as an actix middleware.
pub(super) struct MiddlewareFn<F> {
    inner: Arc<F>,
}

impl<F> MiddlewareFn<F>
where
    F: for<'r> CallableAsyncFn<'r>,
{
    pub fn new(inner: F) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }
}

impl<S, F> Transform<S, ServiceRequest> for MiddlewareFn<F>
where
    S: Service<ServiceRequest, Response = ServiceResponse<BoxBody>, Error = Error> + 'static,
    S::Future: 'static,
    F: 'static + for<'r> CallableAsyncFn<'r>,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type Transform = MiddlewareFnWrapper<S, F>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        futures::future::ok(MiddlewareFnWrapper {
            service: Arc::new(service),
            inner: self.inner.clone(),
        })
    }
}

pub(super) struct MiddlewareFnWrapper<S, F> {
    service: Arc<S>,
    inner: Arc<F>,
}

impl<S, F> Service<ServiceRequest> for MiddlewareFnWrapper<S, F>
where
    S: Service<ServiceRequest, Response = ServiceResponse<BoxBody>, Error = Error> + 'static,
    S::Future: 'static,
    F: 'static + for<'r> CallableAsyncFn<'r>,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = S::Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&self, ct: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ct)
    }

    fn call(&self, mut req: ServiceRequest) -> Self::Future {
        let inner = self.inner.clone();
        let service = self.service.clone();

        async move {
            match inner.call(&mut req).await {
                Err(e) => Ok(
                    req.into_response(HttpResponse::InternalServerError().body(format!("{:?}", e)))
                ),
                Ok(_) => service.call(req).await,
            }
        }
        .boxed_local()
    }
}
