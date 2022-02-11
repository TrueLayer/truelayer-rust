use crate::{
    apis::{merchant_accounts::MerchantAccount, TrueLayerClientInner},
    Error,
};
use serde::Deserialize;
use std::sync::Arc;
use urlencoding::encode;

/// TrueLayer Merchant Accounts APIs client.
#[derive(Clone, Debug)]
pub struct MerchantAccountsApi {
    inner: Arc<TrueLayerClientInner>,
}

impl MerchantAccountsApi {
    pub(crate) fn new(inner: Arc<TrueLayerClientInner>) -> Self {
        Self { inner }
    }

    /// Lists all merchant accounts.
    #[tracing::instrument(name = "List Merchant Accounts", skip(self))]
    pub async fn list(&self) -> Result<Vec<MerchantAccount>, Error> {
        let res: ListResponse<_> = self
            .inner
            .client
            .get(
                self.inner
                    .environment
                    .payments_url()
                    .join("/merchant-accounts")
                    .unwrap(),
            )
            .send()
            .await?
            .json()
            .await?;

        Ok(res.items)
    }

    /// Gets the details of an existing merchant account.
    ///
    /// If there's no merchant account with the given id, `None` is returned.
    #[tracing::instrument(name = "Get Merchant Account by ID", skip(self))]
    pub async fn get_by_id(&self, id: &str) -> Result<Option<MerchantAccount>, Error> {
        let res = self
            .inner
            .client
            .get(
                self.inner
                    .environment
                    .payments_url()
                    .join(&format!("/merchant-accounts/{}", encode(id)))
                    .unwrap(),
            )
            .send()
            .await
            .map_err(Error::from);

        // Return `None` if the server returned 404
        let merchant_account = match res {
            Ok(body) => Some(body.json().await?),
            Err(Error::ApiError(api_error)) if api_error.status == 404 => None,
            Err(e) => return Err(e),
        };

        Ok(merchant_account)
    }
}

#[derive(Deserialize)]
struct ListResponse<T> {
    pub items: Vec<T>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        apis::{
            auth::Credentials,
            payments::{AccountIdentifier, Currency},
        },
        authenticator::Authenticator,
        client::Environment,
        middlewares::error_handling::ErrorHandlingMiddleware,
    };
    use reqwest::Url;
    use serde_json::json;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    async fn mock_client_and_server() -> (MerchantAccountsApi, MockServer) {
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

        (MerchantAccountsApi::new(Arc::new(inner)), mock_server)
    }

    #[tokio::test]
    async fn list() {
        let (api, mock_server) = mock_client_and_server().await;

        Mock::given(method("GET"))
            .and(path("/merchant-accounts"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [
                    {
                        "id": "merchant-account-id",
                        "currency": "GBP",
                        "account_identifiers": [
                            {
                                "type": "sort_code_account_number",
                                "sort_code": "sort-code",
                                "account_number": "account-number"
                            }
                        ],
                        "available_balance_in_minor": 100,
                        "current_balance_in_minor": 200,
                        "account_holder_name": "Mr. Holder"
                    }
                ]
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let merchant_accounts = api.list().await.unwrap();

        assert_eq!(
            merchant_accounts,
            vec![MerchantAccount {
                id: "merchant-account-id".to_string(),
                currency: Currency::Gbp,
                account_identifiers: vec![AccountIdentifier::SortCodeAccountNumber {
                    sort_code: "sort-code".to_string(),
                    account_number: "account-number".to_string()
                }],
                available_balance_in_minor: 100,
                current_balance_in_minor: 200,
                account_holder_name: "Mr. Holder".to_string()
            }]
        );
    }

    #[tokio::test]
    async fn list_empty() {
        let (api, mock_server) = mock_client_and_server().await;

        Mock::given(method("GET"))
            .and(path("/merchant-accounts"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": []
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let merchant_accounts = api.list().await.unwrap();

        assert_eq!(merchant_accounts, vec![]);
    }

    #[tokio::test]
    async fn get_by_id() {
        let (api, mock_server) = mock_client_and_server().await;

        let merchant_account_id = "merchant-account-id".to_string();
        Mock::given(method("GET"))
            .and(path(format!("/merchant-accounts/{}", merchant_account_id)))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": merchant_account_id,
                "currency": "GBP",
                "account_identifiers": [
                    {
                        "type": "sort_code_account_number",
                        "sort_code": "sort-code",
                        "account_number": "account-number"
                    }
                ],
                "available_balance_in_minor": 100,
                "current_balance_in_minor": 200,
                "account_holder_name": "Mr. Holder"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let merchant_account = api.get_by_id(&merchant_account_id).await.unwrap();

        assert_eq!(
            merchant_account,
            Some(MerchantAccount {
                id: "merchant-account-id".to_string(),
                currency: Currency::Gbp,
                account_identifiers: vec![AccountIdentifier::SortCodeAccountNumber {
                    sort_code: "sort-code".to_string(),
                    account_number: "account-number".to_string()
                }],
                available_balance_in_minor: 100,
                current_balance_in_minor: 200,
                account_holder_name: "Mr. Holder".to_string()
            })
        );
    }

    #[tokio::test]
    async fn get_by_id_not_found() {
        let (api, mock_server) = mock_client_and_server().await;

        Mock::given(method("GET"))
            .and(path("/merchant-accounts/merchant-account-id"))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&mock_server)
            .await;

        let merchant_account = api.get_by_id("merchant-account-id").await.unwrap();

        assert_eq!(merchant_account, None);
    }
}
