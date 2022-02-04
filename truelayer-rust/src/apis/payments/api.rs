use crate::{
    apis::{
        payments::{CreatePaymentRequest, CreatePaymentResponse, Payment},
        TrueLayerClientInner,
    },
    common::IDEMPOTENCY_KEY_HEADER,
    Error,
};
use reqwest::Url;
use std::sync::Arc;
use uuid::Uuid;

/// TrueLayer payments APIs client.
#[derive(Clone, Debug)]
pub struct PaymentsApi {
    inner: Arc<TrueLayerClientInner>,
}

impl PaymentsApi {
    pub(crate) fn new(inner: Arc<TrueLayerClientInner>) -> Self {
        Self { inner }
    }

    /// Creates a new payment.
    ///
    /// See documentation of [`TrueLayerClient`](crate::client::TrueLayerClient)
    /// for more details on the idempotency key.
    #[tracing::instrument(
        name = "Create Payment",
        skip(self, create_payment_request),
        fields(
            amount_in_minor = create_payment_request.amount_in_minor,
            currency = %create_payment_request.currency,
        )
    )]
    pub async fn create(
        &self,
        create_payment_request: &CreatePaymentRequest,
    ) -> Result<CreatePaymentResponse, Error> {
        // Generate a new random idempotency-key for this request
        let idempotency_key = Uuid::new_v4();

        let res = self
            .inner
            .client
            .post(
                self.inner
                    .environment
                    .payments_url()
                    .join("/payments")
                    .unwrap(),
            )
            .header(IDEMPOTENCY_KEY_HEADER, idempotency_key.to_string())
            .json(create_payment_request)
            .send()
            .await?
            .json()
            .await?;

        Ok(res)
    }

    /// Gets the details of an existing payment.
    ///
    /// If there's no payment with the given id, `None` is returned.
    #[tracing::instrument(name = "Get Payment by ID", skip(self))]
    pub async fn get_by_id(&self, id: &str) -> Result<Option<Payment>, Error> {
        let res = self
            .inner
            .client
            .get(
                self.inner
                    .environment
                    .payments_url()
                    .join(&format!("/payments/{}", id))
                    .unwrap(),
            )
            .send()
            .await
            .map_err(Error::from);

        // Return `None` if the server returned 404
        let payment = match res {
            Ok(body) => Some(body.json().await?),
            Err(Error::ApiError(api_error)) if api_error.status == 404 => None,
            Err(e) => return Err(e),
        };

        Ok(payment)
    }

    /// Creates a link to the TrueLayer Hosted Payments Page.
    ///
    /// Note that the `return_uri` must be configured in your TrueLayer console.
    pub async fn get_hosted_payments_page_link(
        &self,
        payment_id: &str,
        payment_token: &str,
        return_uri: &str,
    ) -> Url {
        let mut new_uri = self.inner.environment.hpp_url().join("/payments").unwrap();

        new_uri.set_fragment(Some(&format!(
            "payment_id={}&payment_token={}&return_uri={}",
            payment_id, payment_token, return_uri
        )));

        new_uri
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        apis::{
            auth::Credentials,
            payments::{
                Beneficiary, Currency, PaymentMethod, PaymentStatus, ProviderSelection, User,
            },
        },
        authenticator::Authenticator,
        client::Environment,
        middlewares::error_handling::ErrorHandlingMiddleware,
    };
    use chrono::Utc;
    use reqwest::Url;
    use serde_json::json;
    use wiremock::{
        matchers::{body_partial_json, header_exists, method, path},
        Mock, MockServer, ResponseTemplate,
    };

    async fn mock_client_and_server() -> (TrueLayerClientInner, MockServer) {
        let mock_server = MockServer::start().await;

        let credentials = Credentials::ClientCredentials {
            client_id: "client-id".to_string(),
            client_secret: "client-secret".to_string(),
            scope: "mock".to_string(),
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
        let api = PaymentsApi::new(Arc::new(inner));

        Mock::given(method("POST"))
            .and(path("/payments"))
            .and(header_exists(IDEMPOTENCY_KEY_HEADER))
            .and(body_partial_json(json!({
                "amount_in_minor": 100,
                "currency": "GBP",
                "payment_method": {
                    "type": "bank_transfer",
                    "provider_selection": {
                        "type": "user_selected"
                    },
                    "beneficiary": {
                        "type": "merchant_account",
                        "merchant_account_id": "merchant-account-id"
                    },
                },
                "user": {
                    "id": "user-id"
                }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "payment-id",
                "payment_token": "payment-token",
                "user": {
                    "id": "user-id"
                }
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let res = api
            .create(&CreatePaymentRequest {
                amount_in_minor: 100,
                currency: Currency::Gbp,
                payment_method: PaymentMethod::BankTransfer {
                    provider_selection: ProviderSelection::UserSelected { filter: None },
                    beneficiary: Beneficiary::MerchantAccount {
                        merchant_account_id: "merchant-account-id".to_string(),
                        account_holder_name: None,
                    },
                },
                user: User {
                    id: Some("user-id".to_string()),
                    name: None,
                    email: None,
                    phone: None,
                },
            })
            .await
            .unwrap();

        assert_eq!(res.id, "payment-id");
        assert_eq!(res.payment_token, "payment-token");
        assert_eq!(res.user.id, "user-id");
    }

    #[tokio::test]
    async fn get_by_id_successful() {
        let (inner, mock_server) = mock_client_and_server().await;
        let api = PaymentsApi::new(Arc::new(inner));

        let payment_id = "some-known-payment-id";
        Mock::given(method("GET"))
            .and(path(format!("/payments/{}", payment_id)))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": payment_id,
                "amount_in_minor": 100,
                "currency": "GBP",
                "payment_method": {
                    "type": "bank_transfer",
                    "provider_selection": {
                        "type": "user_selected"
                    },
                    "beneficiary": {
                        "type": "merchant_account",
                        "merchant_account_id": "merchant-account-id",
                    }
                },
                "user": {
                    "id": "user-id"
                },
                "created_at": Utc::now(),
                "status": "authorization_required",
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let payment = api.get_by_id(payment_id).await.unwrap().unwrap();

        assert_eq!(payment.id, payment_id);
        assert_eq!(payment.amount_in_minor, 100);
        assert_eq!(payment.currency, Currency::Gbp);
        assert_eq!(
            payment.payment_method,
            PaymentMethod::BankTransfer {
                provider_selection: ProviderSelection::UserSelected { filter: None },
                beneficiary: Beneficiary::MerchantAccount {
                    merchant_account_id: "merchant-account-id".to_string(),
                    account_holder_name: None
                }
            }
        );
        assert_eq!(
            payment.user,
            User {
                id: Some("user-id".to_string()),
                name: None,
                email: None,
                phone: None
            }
        );
        assert_eq!(payment.status, PaymentStatus::AuthorizationRequired);
    }

    #[tokio::test]
    async fn get_by_id_not_found() {
        let (inner, mock_server) = mock_client_and_server().await;
        let api = PaymentsApi::new(Arc::new(inner));

        Mock::given(method("GET"))
            .and(path("/payments/non-existent"))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&mock_server)
            .await;

        assert!(api.get_by_id("non-existent").await.unwrap().is_none());
    }
}
