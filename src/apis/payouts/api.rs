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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        apis::{
            auth::Credentials,
            payments::{AccountIdentifier, Currency},
            payouts::{PayoutBeneficiary, PayoutStatus},
        },
        authenticator::Authenticator,
        client::Environment,
        middlewares::error_handling::ErrorHandlingMiddleware,
    };
    use chrono::{TimeZone, Utc};
    use serde_json::json;
    use url::Url;
    use wiremock::{
        matchers::{body_partial_json, header_exists, method, path},
        Mock, MockServer, ResponseTemplate,
    };

    async fn mock_client_and_server() -> (TrueLayerClientInner, MockServer) {
        let mock_server = MockServer::start().await;

        let credentials = Credentials::ClientCredentials {
            client_id: "client-id".into(),
            client_secret: "client-secret".into(),
            scope: "mock".into(),
        };

        let authenticator = Authenticator::new(
            reqwest::Client::new().into(),
            Url::parse(&mock_server.uri()).unwrap(),
            credentials,
        );

        let inner = TrueLayerClientInner {
            client: reqwest_middleware::ClientBuilder::new(reqwest::Client::new())
                .with(ErrorHandlingMiddleware)
                .build(),
            authenticator,
            environment: Environment::from_single_url(&Url::parse(&mock_server.uri()).unwrap()),
        };

        (inner, mock_server)
    }

    #[tokio::test]
    async fn create() {
        let (inner, mock_server) = mock_client_and_server().await;
        let api = PayoutsApi::new(Arc::new(inner));

        Mock::given(method("POST"))
            .and(path("/payouts"))
            .and(header_exists(IDEMPOTENCY_KEY_HEADER))
            .and(body_partial_json(json!({
                "merchant_account_id": "merchant-account-id",
                "amount_in_minor": 100,
                "currency": "GBP",
                "beneficiary": {
                    "type": "external_account",
                    "account_holder_name": "Mr. Holder",
                    "account_identifier": {
                        "type": "iban",
                        "iban": "some-iban"
                    },
                    "reference": "some-reference"
                }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "payout-id"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let res = api
            .create(&CreatePayoutRequest {
                merchant_account_id: "merchant-account-id".to_string(),
                amount_in_minor: 100,
                currency: Currency::Gbp,
                beneficiary: PayoutBeneficiary::ExternalAccount {
                    account_holder_name: "Mr. Holder".to_string(),
                    account_identifier: AccountIdentifier::Iban {
                        iban: "some-iban".to_string(),
                    },
                    reference: "some-reference".to_string(),
                },
            })
            .await
            .unwrap();

        assert_eq!(res.id, "payout-id");
    }

    #[tokio::test]
    async fn get_by_id_successful() {
        let (inner, mock_server) = mock_client_and_server().await;
        let api = PayoutsApi::new(Arc::new(inner));

        let payout_id = "some-known-payout-id";
        Mock::given(method("GET"))
            .and(path(format!("/payouts/{}", payout_id)))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": payout_id,
                "merchant_account_id": "some-merchant-account-id",
                "amount_in_minor": 100,
                "currency": "GBP",
                "beneficiary": {
                    "type": "external_account",
                    "account_holder_name": "Mr. Holder",
                    "account_identifier": {
                        "type": "iban",
                        "iban": "some-iban"
                    },
                    "reference": "some-reference"
                },
                "status": "executed",
                "created_at": "2022-04-01T00:00:00Z",
                "executed_at": "2022-04-01T00:00:00Z"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let payout = api.get_by_id(payout_id).await.unwrap().unwrap();

        assert_eq!(payout.id, payout_id);
        assert_eq!(
            payout.merchant_account_id,
            "some-merchant-account-id".to_string()
        );
        assert_eq!(payout.amount_in_minor, 100);
        assert_eq!(payout.currency, Currency::Gbp);
        assert_eq!(
            payout.beneficiary,
            PayoutBeneficiary::ExternalAccount {
                account_holder_name: "Mr. Holder".to_string(),
                account_identifier: AccountIdentifier::Iban {
                    iban: "some-iban".to_string(),
                },
                reference: "some-reference".to_string(),
            }
        );
        assert_eq!(payout.created_at, Utc.ymd(2022, 4, 1).and_hms(0, 0, 0));
        assert_eq!(
            payout.status,
            PayoutStatus::Executed {
                executed_at: Utc.ymd(2022, 4, 1).and_hms(0, 0, 0)
            }
        );
    }

    #[tokio::test]
    async fn get_by_id_not_found() {
        let (inner, mock_server) = mock_client_and_server().await;
        let api = PayoutsApi::new(Arc::new(inner));

        Mock::given(method("GET"))
            .and(path("/payouts/non-existent"))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&mock_server)
            .await;

        assert!(api.get_by_id("non-existent").await.unwrap().is_none());
    }
}
