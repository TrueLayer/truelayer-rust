use crate::common::mock_server::MockServerConfiguration;
use actix_web::body::BoxBody;
use actix_web::dev::{Payload, Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::error::PayloadError;
use actix_web::web::{Bytes, BytesMut};
use actix_web::{Error, HttpMessage, HttpResponse};
use anyhow::anyhow;
use futures::future::{LocalBoxFuture, Ready};
use futures::{FutureExt, StreamExt, TryFutureExt, TryStreamExt};
use std::cell::RefCell;
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll};

/// Middleware to check that all the requests contain the right user agent header
pub(super) async fn validate_user_agent(
    req: ServiceRequest,
) -> Result<ServiceRequest, anyhow::Error> {
    // Check that the User-Agent header is present
    anyhow::ensure!(
        req.headers()
            .get("User-Agent")
            .map(|v| v.to_str())
            .transpose()?
            == Some(concat!("truelayer-rust/", env!("CARGO_PKG_VERSION"))),
        "Invalid User-Agent"
    );

    Ok(req)
}

/// Ensures that the incoming request has an idempotency key set
pub(super) async fn ensure_idempotency_key(
    req: ServiceRequest,
) -> Result<ServiceRequest, anyhow::Error> {
    anyhow::ensure!(
        req.headers()
            .get("Idempotency-Key")
            .map(|v| v.to_str())
            .transpose()?
            .map_or(false, |v| !v.is_empty()),
        "Invalid or missing Idempotency Key"
    );

    Ok(req)
}

/// Validates a full request signature
pub(super) fn validate_signature(
    configuration: MockServerConfiguration,
    require_idempotency_key: bool,
) -> impl Fn(ServiceRequest) -> LocalBoxFuture<'static, Result<ServiceRequest, anyhow::Error>> {
    let configuration = Arc::new(configuration);

    move |mut req: ServiceRequest| {
        let configuration = configuration.clone();

        Box::pin(async move {
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
                truelayer_signing::verify_with_pem(configuration.certificate_public_key.as_slice())
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
            verifier.body(&body).verify(signature)?;

            // Put the body back into the request so that it can be consumed by other middlewares
            req.set_payload(Payload::Stream {
                payload: futures::stream::once(
                    async move { Ok::<_, PayloadError>(Bytes::from(body)) },
                )
                .boxed(),
            });

            Ok(req)
        })
    }
}

/// Wrapper around a function to act as an actix middleware.
pub(super) struct MiddlewareFn<F> {
    inner: Arc<F>,
}

impl<F, FnFut> MiddlewareFn<F>
where
    F: Fn(ServiceRequest) -> FnFut,
    FnFut: Future<Output = Result<ServiceRequest, anyhow::Error>> + 'static,
{
    pub fn new(inner: F) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }
}

impl<S, F, FnFut> Transform<S, ServiceRequest> for MiddlewareFn<F>
where
    S: Service<ServiceRequest, Response = ServiceResponse<BoxBody>, Error = Error> + 'static,
    S::Future: 'static,
    F: Fn(ServiceRequest) -> FnFut,
    FnFut: Future<Output = Result<ServiceRequest, anyhow::Error>> + 'static,
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

impl<S, F, FnFut> Service<ServiceRequest> for MiddlewareFnWrapper<S, F>
where
    S: Service<ServiceRequest, Response = ServiceResponse<BoxBody>, Error = Error> + 'static,
    S::Future: 'static,
    F: Fn(ServiceRequest) -> FnFut,
    FnFut: Future<Output = Result<ServiceRequest, anyhow::Error>> + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = S::Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&self, ct: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ct)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = self.service.clone();

        (self.inner)(req)
            .then(|res| async move {
                match res {
                    Err(e) => Ok(req.into_response(
                        HttpResponse::InternalServerError().body(format!("{:?}", e)),
                    )),
                    Ok(req) => service.call(req).await,
                }
            })
            .boxed_local()
    }
}
