use std::sync::Arc;

use urlencoding::encode;

use crate::{apis::TrueLayerClientInner, Error};

use super::model::Provider;

/// TrueLayer payments APIs client.
#[derive(Clone, Debug)]
pub struct PaymentsProvidersApi {
    inner: Arc<TrueLayerClientInner>,
}

impl PaymentsProvidersApi {
    pub(crate) fn new(inner: Arc<TrueLayerClientInner>) -> Self {
        Self { inner }
    }

    /// Gets the details of a payments provider.
    ///
    /// If there's no provider with the given id, `None` is returned.
    ///
    /// This client always sets the `client_id` query parameter from the client configuration.
    /// Thus, only provider capabilities which are available to the `client_id` will be returned.
    #[tracing::instrument(name = "Get Provider by ID", skip(self))]
    pub async fn get_by_id(&self, id: &str) -> Result<Option<Provider>, Error> {
        let res = self
            .inner
            .client
            .get(
                self.inner
                    .environment
                    .payments_url()
                    .join(&format!("/payments-providers/{}", encode(id)))
                    .unwrap(),
            )
            .query(&[("client_id", &self.inner.authenticator.client_id)])
            .send()
            .await
            .map_err(Error::from);

        // Return `None` if the server returned 404
        let provider = match res {
            Ok(body) => Some(body.json().await?),
            Err(Error::ApiError(api_error)) if api_error.status == 404 => None,
            Err(e) => return Err(e),
        };

        Ok(provider)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use reqwest::Url;
    use serde_json::json;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    use crate::{
        apis::{
            auth::Credentials,
            payments::CountryCode,
            payments_providers::{
                api::PaymentsProvidersApi,
                model::{capabilities, Capabilities, PaymentScheme},
            },
            TrueLayerClientInner,
        },
        authenticator::Authenticator,
        client::Environment,
        middlewares::error_handling::ErrorHandlingMiddleware,
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
    async fn get_by_id_successful() {
        let (inner, mock_server) = mock_client_and_server().await;
        let api = PaymentsProvidersApi::new(Arc::new(inner));

        let provider_id = "some-known-payment-id";
        Mock::given(method("GET"))
            .and(path(format!("/payments-providers/{provider_id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": provider_id,
                "display_name": "Mock Payments Provider",
                "icon_uri": "https://icon.uri",
                "logo_uri": "https://logo.uri",
                "bg_color": "#FFFFFF",
                "country_code": "ES",
                "capabilities": {
                    "payments": {
                        "bank_transfer": {
                            "release_channel": "general_availability",
                            "schemes": [
                                {
                                    "id": "sepa_credit_transfer"
                                },
                                {
                                    "id": "sepa_credit_transfer_instant"
                                }
                            ]
                        }
                    }
                }
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let provider = api.get_by_id(provider_id).await.unwrap().unwrap();

        assert_eq!(provider.id, provider_id);
        assert_eq!(provider.display_name, Some("Mock Payments Provider".into()));
        assert_eq!(provider.icon_uri, Some("https://icon.uri".into()));
        assert_eq!(provider.logo_uri, Some("https://logo.uri".into()));
        assert_eq!(provider.bg_color, Some("#FFFFFF".into()));
        assert_eq!(provider.country_code, Some(CountryCode::ES));
        assert_eq!(
            provider.capabilities,
            Capabilities {
                payments: capabilities::Payments {
                    bank_transfer: Some(capabilities::BankTransfer {
                        release_channel: crate::apis::payments::ReleaseChannel::GeneralAvailability,
                        schemes: vec![
                            PaymentScheme {
                                id: "sepa_credit_transfer".into()
                            },
                            PaymentScheme {
                                id: "sepa_credit_transfer_instant".into()
                            }
                        ]
                    })
                }
            }
        );
    }

    #[tokio::test]
    async fn get_by_id_not_found() {
        let (inner, mock_server) = mock_client_and_server().await;
        let api = PaymentsProvidersApi::new(Arc::new(inner));

        Mock::given(method("GET"))
            .and(path("/payments-providers/non-existent"))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&mock_server)
            .await;

        assert!(api.get_by_id("non-existent").await.unwrap().is_none());
    }
}
