use crate::{
    apis::{
        payouts::{CreatePayoutRequest, CreatePayoutResponse, Payout},
        TrueLayerClientInner,
    },
    common::IDEMPOTENCY_KEY_HEADER,
    Error,
};
use std::sync::Arc;
use urlencoding::encode;
use uuid::Uuid;

/// TrueLayer payouts APIs client.
#[derive(Clone, Debug)]
pub struct PayoutsApi {
    inner: Arc<TrueLayerClientInner>,
}

impl PayoutsApi {
    pub(crate) fn new(inner: Arc<TrueLayerClientInner>) -> Self {
        Self { inner }
    }

    /// Payout from one of your merchant accounts.
    #[tracing::instrument(
        name = "Create Payout",
        skip(self, create_payout_request),
        fields(
            amount_in_minor = create_payout_request.amount_in_minor,
            currency = % create_payout_request.currency,
        )
    )]
    pub async fn create(
        &self,
        create_payout_request: &CreatePayoutRequest,
    ) -> Result<CreatePayoutResponse, Error> {
        // Generate a new random idempotency-key for this request
        let idempotency_key = Uuid::new_v4();

        let res = self
            .inner
            .client
            .post(
                self.inner
                    .environment
                    .payments_url()
                    .join("/payouts")
                    .unwrap(),
            )
            .header(IDEMPOTENCY_KEY_HEADER, idempotency_key.to_string())
            .json(create_payout_request)
            .send()
            .await?
            .json()
            .await?;

        Ok(res)
    }

    /// Gets the details of an existing payout.
    ///
    /// If there's no payout with the given id, `None` is returned.
    #[tracing::instrument(name = "Get Payout by ID", skip(self))]
    pub async fn get_by_id(&self, id: &str) -> Result<Option<Payout>, Error> {
        let res = self
            .inner
            .client
            .get(
                self.inner
                    .environment
                    .payments_url()
                    .join(&format!("/payouts/{}", encode(id)))
                    .unwrap(),
            )
            .send()
            .await
            .map_err(Error::from);

        // Return `None` if the server returned 404
        let payout = match res {
            Ok(body) => Some(body.json().await?),
            Err(Error::ApiError(api_error)) if api_error.status == 404 => None,
            Err(e) => return Err(e),
        };

        Ok(payout)
    }
}
