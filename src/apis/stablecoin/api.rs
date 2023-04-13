use crate::{apis::TrueLayerClientInner, common::IDEMPOTENCY_KEY_HEADER, Error};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

/// TrueLayer payments APIs client.
#[derive(Clone, Debug)]
pub struct StablecoinApi {
    inner: Arc<TrueLayerClientInner>,
}

impl StablecoinApi {
    pub(crate) fn new(inner: Arc<TrueLayerClientInner>) -> Self {
        Self { inner }
    }

    /// Make a request to request signature testing endpoint.
    #[tracing::instrument(name = "Call Request Signature Testing Endpoint", skip(self))]
    pub async fn test_signature(&self) -> Result<(), Error> {
        // Generate a new random idempotency-key for this request
        let idempotency_key = Uuid::new_v4();

        self.inner
            .client
            .post(
                self.inner
                    .environment
                    .stablecoin_url()
                    .join("/v1/test-signature")
                    .unwrap(),
            )
            .header(IDEMPOTENCY_KEY_HEADER, idempotency_key.to_string())
            .json(&json!({"foo": "bar"}))
            .send()
            .await?;

        Ok(())
    }
}
