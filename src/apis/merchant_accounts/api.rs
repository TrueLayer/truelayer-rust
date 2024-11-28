use crate::{
    apis::{
        merchant_accounts::{
            ListPaymentSourcesRequest, ListTransactionsRequest, MerchantAccount,
            SetupSweepingRequest, SweepingSettings, Transaction,
        },
        payments::PaymentSource,
        TrueLayerClientInner,
    },
    common::IDEMPOTENCY_KEY_HEADER,
    Error,
};
use serde::Deserialize;
use std::sync::Arc;
use urlencoding::encode;
use uuid::Uuid;

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
    pub async fn get_by_id(
        &self,
        merchant_account_id: &str,
    ) -> Result<Option<MerchantAccount>, Error> {
        let res = self
            .inner
            .client
            .get(
                self.inner
                    .environment
                    .payments_url()
                    .join(&format!(
                        "/merchant-accounts/{}",
                        encode(merchant_account_id)
                    ))
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

    /// Set the automatic sweeping settings for a merchant account.
    /// At regular intervals, any available balance in excess of the configured
    /// `max_amount_in_minor` is withdrawn to a pre-configured IBAN.
    #[tracing::instrument(
        name = "Setup Merchant Account Sweeping",
        skip(self, merchant_account_id, request),
        fields(
            merchant_account_id = %merchant_account_id,
            amount_in_minor = %request.max_amount_in_minor,
            currency = %request.currency
        )
    )]
    pub async fn setup_sweeping(
        &self,
        merchant_account_id: &str,
        request: &SetupSweepingRequest,
    ) -> Result<(), Error> {
        // Generate a new random idempotency-key for this request
        let idempotency_key = Uuid::new_v4();

        self.inner
            .client
            .post(
                self.inner
                    .environment
                    .payments_url()
                    .join(&format!(
                        "/merchant-accounts/{}/sweeping",
                        merchant_account_id
                    ))
                    .unwrap(),
            )
            .header(IDEMPOTENCY_KEY_HEADER, idempotency_key.to_string())
            .json(request)
            .send()
            .await?;

        Ok(())
    }

    /// Disable automatic sweeping for a merchant account.
    #[tracing::instrument(name = "Disable Merchant Account Sweeping", skip(self))]
    pub async fn disable_sweeping(&self, merchant_account_id: &str) -> Result<(), Error> {
        // Generate a new random idempotency-key for this request
        let idempotency_key = Uuid::new_v4();

        self.inner
            .client
            .delete(
                self.inner
                    .environment
                    .payments_url()
                    .join(&format!(
                        "/merchant-accounts/{}/sweeping",
                        merchant_account_id
                    ))
                    .unwrap(),
            )
            .header(IDEMPOTENCY_KEY_HEADER, idempotency_key.to_string())
            .send()
            .await?;

        Ok(())
    }

    /// Gets the currently active automatic sweeping configuration of a merchant account.
    ///
    /// If there's no merchant account with the given id, or if it has not enabled sweeping,
    /// `None` is returned.
    #[tracing::instrument(name = "Get Merchant Account Sweeping Settings", skip(self))]
    pub async fn get_sweeping_settings(
        &self,
        merchant_account_id: &str,
    ) -> Result<Option<SweepingSettings>, Error> {
        let res = self
            .inner
            .client
            .get(
                self.inner
                    .environment
                    .payments_url()
                    .join(&format!(
                        "/merchant-accounts/{}/sweeping",
                        encode(merchant_account_id)
                    ))
                    .unwrap(),
            )
            .send()
            .await
            .map_err(Error::from);

        // Return `None` if the server returned 404
        let settings = match res {
            Ok(body) => Some(body.json().await?),
            Err(Error::ApiError(api_error)) if api_error.status == 404 => None,
            Err(e) => return Err(e),
        };

        Ok(settings)
    }

    /// Gets the transactions of a single merchant account.
    #[tracing::instrument(name = "List Transactions", skip(self, request))]
    pub async fn list_transactions(
        &self,
        merchant_account_id: &str,
        request: &ListTransactionsRequest,
    ) -> Result<Vec<Transaction>, Error> {
        let res: ListResponse<_> = self
            .inner
            .client
            .get(
                self.inner
                    .environment
                    .payments_url()
                    .join(&format!(
                        "/merchant-accounts/{}/transactions",
                        merchant_account_id
                    ))
                    .unwrap(),
            )
            .query(request)
            .send()
            .await?
            .json()
            .await?;

        Ok(res.items)
    }

    /// Gets the payment sources from which the merchant account has received payment.
    #[tracing::instrument(
        name = "List Payment Sources",
        skip(self, request),
        fields(
            user_id = %request.user_id
        )
    )]
    pub async fn list_payment_sources(
        &self,
        merchant_account_id: &str,
        request: &ListPaymentSourcesRequest,
    ) -> Result<Vec<PaymentSource>, Error> {
        let res: ListResponse<_> = self
            .inner
            .client
            .get(
                self.inner
                    .environment
                    .payments_url()
                    .join(&format!(
                        "/merchant-accounts/{}/payment-sources",
                        merchant_account_id
                    ))
                    .unwrap(),
            )
            .query(request)
            .send()
            .await?
            .json()
            .await?;

        Ok(res.items)
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
            merchant_accounts::{
                SweepingFrequency, TransactionPayinStatus, TransactionPayoutContextCode,
                TransactionPayoutStatus, TransactionType,
            },
            payments::{AccountIdentifier, Currency, ExternalPaymentRemitter},
            payouts::PayoutBeneficiary,
        },
        authenticator::Authenticator,
        client::Environment,
        middlewares::error_handling::ErrorHandlingMiddleware,
    };
    use chrono::{SecondsFormat, Utc};
    use reqwest::Url;
    use serde_json::json;
    use wiremock::{
        matchers::{body_partial_json, method, path, query_param},
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

    #[tokio::test]
    async fn setup_sweeping() {
        let (api, mock_server) = mock_client_and_server().await;

        let merchant_account_id = "merchant-account-id".to_string();
        Mock::given(method("POST"))
            .and(path(format!(
                "/merchant-accounts/{}/sweeping",
                merchant_account_id
            )))
            .and(body_partial_json(json!({
                "max_amount_in_minor": 100,
                "currency": "GBP",
                "frequency": "daily"
            })))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&mock_server)
            .await;

        api.setup_sweeping(
            &merchant_account_id,
            &SetupSweepingRequest {
                max_amount_in_minor: 100,
                currency: Currency::Gbp,
                frequency: SweepingFrequency::Daily,
            },
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn setup_sweeping_account_not_found() {
        let (api, mock_server) = mock_client_and_server().await;

        let merchant_account_id = "merchant-account-id".to_string();
        Mock::given(method("POST"))
            .and(path(format!(
                "/merchant-accounts/{}/sweeping",
                merchant_account_id
            )))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&mock_server)
            .await;

        let res = api
            .setup_sweeping(
                &merchant_account_id,
                &SetupSweepingRequest {
                    max_amount_in_minor: 100,
                    currency: Currency::Gbp,
                    frequency: SweepingFrequency::Daily,
                },
            )
            .await;

        // Expect an error
        assert!(matches!(res, Err(Error::ApiError(e)) if e.status == 404));
    }

    #[tokio::test]
    async fn disable_sweeping() {
        let (api, mock_server) = mock_client_and_server().await;

        let merchant_account_id = "merchant-account-id".to_string();
        Mock::given(method("DELETE"))
            .and(path(format!(
                "/merchant-accounts/{}/sweeping",
                merchant_account_id
            )))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&mock_server)
            .await;

        api.disable_sweeping(&merchant_account_id).await.unwrap();
    }

    #[tokio::test]
    async fn disable_sweeping_account_not_found() {
        let (api, mock_server) = mock_client_and_server().await;

        let merchant_account_id = "merchant-account-id".to_string();
        Mock::given(method("DELETE"))
            .and(path(format!(
                "/merchant-accounts/{}/sweeping",
                merchant_account_id
            )))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&mock_server)
            .await;

        let res = api.disable_sweeping(&merchant_account_id).await;

        // Expect an error
        assert!(matches!(res, Err(Error::ApiError(e)) if e.status == 404));
    }

    #[tokio::test]
    async fn get_sweeping_settings() {
        let (api, mock_server) = mock_client_and_server().await;

        let merchant_account_id = "merchant-account-id".to_string();
        Mock::given(method("GET"))
            .and(path(format!(
                "/merchant-accounts/{}/sweeping",
                merchant_account_id
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "max_amount_in_minor": 100,
                "currency": "GBP",
                "frequency": "weekly",
                "destination": {
                    "type": "iban",
                    "iban": "some-iban"
                }
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let settings = api
            .get_sweeping_settings(&merchant_account_id)
            .await
            .unwrap();

        assert_eq!(
            settings,
            Some(SweepingSettings {
                max_amount_in_minor: 100,
                currency: Currency::Gbp,
                frequency: SweepingFrequency::Weekly,
                destination: AccountIdentifier::Iban {
                    iban: "some-iban".into()
                }
            })
        );
    }

    #[tokio::test]
    async fn get_sweeping_settings_not_found() {
        let (api, mock_server) = mock_client_and_server().await;

        let merchant_account_id = "merchant-account-id".to_string();
        Mock::given(method("GET"))
            .and(path(format!(
                "/merchant-accounts/{}/sweeping",
                merchant_account_id
            )))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&mock_server)
            .await;

        let sweeping_settings = api
            .get_sweeping_settings(&merchant_account_id)
            .await
            .unwrap();

        assert_eq!(sweeping_settings, None);
    }

    #[tokio::test]
    async fn list_payment_sources() {
        let (api, mock_server) = mock_client_and_server().await;

        let merchant_account_id = "merchant-account-id".to_string();
        let user_id = "user-id".to_string();
        Mock::given(method("GET"))
            .and(path(format!(
                "/merchant-accounts/{}/payment-sources",
                merchant_account_id
            )))
            .and(query_param("user_id", &user_id))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [
                    {
                        "id": "payment-source-id",
                        "user_id": "payment-source-user-id",
                        "account_identifiers": [
                            {
                                "type": "sort_code_account_number",
                                "sort_code": "sort-code",
                                "account_number": "account-number"
                            }
                        ],
                        "account_holder_name": "Mr. Holder"
                    }
                ]
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let payment_sources = api
            .list_payment_sources(&merchant_account_id, &ListPaymentSourcesRequest { user_id })
            .await
            .unwrap();

        assert_eq!(
            payment_sources,
            vec![PaymentSource {
                id: "payment-source-id".to_string(),
                user_id: Some("payment-source-user-id".to_string()),
                account_identifiers: vec![AccountIdentifier::SortCodeAccountNumber {
                    sort_code: "sort-code".to_string(),
                    account_number: "account-number".to_string()
                }],
                account_holder_name: Some("Mr. Holder".to_string())
            }]
        );
    }

    #[tokio::test]
    async fn list_payment_sources_empty() {
        let (api, mock_server) = mock_client_and_server().await;

        let merchant_account_id = "merchant-account-id".to_string();
        let user_id = "user-id".to_string();
        Mock::given(method("GET"))
            .and(path(format!(
                "/merchant-accounts/{}/payment-sources",
                merchant_account_id
            )))
            .and(query_param("user_id", &user_id))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": []
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let payment_sources = api
            .list_payment_sources(&merchant_account_id, &ListPaymentSourcesRequest { user_id })
            .await
            .unwrap();

        assert_eq!(payment_sources, vec![]);
    }

    #[tokio::test]
    async fn list_payment_sources_not_found() {
        let (api, mock_server) = mock_client_and_server().await;

        let merchant_account_id = "merchant-account-id".to_string();
        let user_id = "user-id".to_string();
        Mock::given(method("GET"))
            .and(path(format!(
                "/merchant-accounts/{}/payment-sources",
                merchant_account_id
            )))
            .and(query_param("user_id", &user_id))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&mock_server)
            .await;

        let res = api
            .list_payment_sources(&merchant_account_id, &ListPaymentSourcesRequest { user_id })
            .await;

        // Expect an error
        assert!(matches!(res, Err(Error::ApiError(e)) if e.status == 404));
    }

    #[tokio::test]
    async fn list_transactions() {
        let (api, mock_server) = mock_client_and_server().await;

        let merchant_account_id = "merchant-account-id".to_string();
        let now = Utc::now();
        let now_str = now.to_rfc3339_opts(SecondsFormat::Millis, true);

        Mock::given(method("GET"))
            .and(path(format!(
                "/merchant-accounts/{}/transactions",
                merchant_account_id
            )))
            .and(query_param("from", &now_str))
            .and(query_param("to", &now_str))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [
                    {
                        "id": "transaction-id-1",
                        "currency": "GBP",
                        "amount_in_minor": 100,
                        "type": "merchant_account_payment",
                        "status": "settled",
                        "settled_at": &now,
                        "payment_source": {
                            "id": "payment-source-id",
                            "account_identifiers": [
                                {
                                    "type": "sort_code_account_number",
                                    "sort_code": "sort-code",
                                    "account_number": "account-number"
                                }
                            ],
                            "account_holder_name": "Mr. Holder"
                        },
                        "payment_id": "payment-id"
                    },
                    {
                        "id": "transaction-id-2",
                        "currency": "GBP",
                        "amount_in_minor": 100,
                        "type": "external_payment",
                        "status": "settled",
                        "settled_at": &now,
                        "remitter": {
                            "account_identifier": {
                                "type": "sort_code_account_number",
                                "sort_code": "sort-code",
                                "account_number": "account-number"
                            },
                            "account_holder_name": "Mr. Holder",
                            "reference": "ext-payment-ref"
                        }
                    },
                    {
                        "id": "transaction-id-3",
                        "currency": "GBP",
                        "amount_in_minor": 100,
                        "type": "payout",
                        "status": "pending",
                        "created_at": &now,
                        "beneficiary": {
                            "type": "external_account",
                            "account_identifier": {
                                "type": "sort_code_account_number",
                                "sort_code": "sort-code",
                                "account_number": "account-number"
                            },
                            "account_holder_name": "Mr. Holder",
                            "reference": "payout-reference"
                        },
                        "context_code": "withdrawal",
                        "payout_id": "payout-id-3"
                    },
                    {
                        "id": "transaction-id-4",
                        "currency": "GBP",
                        "amount_in_minor": 100,
                        "type": "payout",
                        "status": "executed",
                        "created_at": &now,
                        "executed_at": &now,
                        "beneficiary": {
                            "type": "payment_source",
                            "user_id": "payout-user-id",
                            "payment_source_id": "payment-source-id",
                            "reference": "payout-reference"
                        },
                        "context_code": "internal",
                        "payout_id": "payout-id-4"
                    }
                ]
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let transactions = api
            .list_transactions(
                &merchant_account_id,
                &ListTransactionsRequest {
                    from: now,
                    to: now,
                    r#type: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(
            transactions,
            vec![
                Transaction {
                    id: "transaction-id-1".into(),
                    currency: Currency::Gbp,
                    amount_in_minor: 100,
                    r#type: TransactionType::MerchantAccountPayment {
                        status: TransactionPayinStatus::Settled,
                        settled_at: now,
                        payment_source: PaymentSource {
                            id: "payment-source-id".into(),
                            user_id: None,
                            account_identifiers: vec![AccountIdentifier::SortCodeAccountNumber {
                                sort_code: "sort-code".to_string(),
                                account_number: "account-number".to_string()
                            }],
                            account_holder_name: Some("Mr. Holder".into())
                        },
                        payment_id: "payment-id".into()
                    }
                },
                Transaction {
                    id: "transaction-id-2".into(),
                    currency: Currency::Gbp,
                    amount_in_minor: 100,
                    r#type: TransactionType::ExternalPayment {
                        status: TransactionPayinStatus::Settled,
                        settled_at: now,
                        remitter: ExternalPaymentRemitter {
                            account_holder_name: "Mr. Holder".into(),
                            account_identifier: AccountIdentifier::SortCodeAccountNumber {
                                sort_code: "sort-code".to_string(),
                                account_number: "account-number".to_string()
                            },
                            reference: "ext-payment-ref".to_string()
                        }
                    }
                },
                Transaction {
                    id: "transaction-id-3".into(),
                    currency: Currency::Gbp,
                    amount_in_minor: 100,
                    r#type: TransactionType::Payout {
                        status: TransactionPayoutStatus::Pending,
                        created_at: now,
                        beneficiary: PayoutBeneficiary::ExternalAccount {
                            account_holder_name: "Mr. Holder".into(),
                            account_identifier: AccountIdentifier::SortCodeAccountNumber {
                                sort_code: "sort-code".to_string(),
                                account_number: "account-number".to_string()
                            },
                            reference: "payout-reference".to_string()
                        },
                        context_code: TransactionPayoutContextCode::Withdrawal,
                        payout_id: "payout-id-3".into()
                    }
                },
                Transaction {
                    id: "transaction-id-4".into(),
                    currency: Currency::Gbp,
                    amount_in_minor: 100,
                    r#type: TransactionType::Payout {
                        status: TransactionPayoutStatus::Executed { executed_at: now },
                        created_at: now,
                        beneficiary: PayoutBeneficiary::PaymentSource {
                            user_id: "payout-user-id".to_string(),
                            payment_source_id: "payment-source-id".to_string(),
                            reference: "payout-reference".to_string()
                        },
                        context_code: TransactionPayoutContextCode::Internal,
                        payout_id: "payout-id-4".into()
                    }
                },
            ]
        );
    }

    #[tokio::test]
    async fn list_transactions_empty() {
        let (api, mock_server) = mock_client_and_server().await;

        let merchant_account_id = "merchant-account-id".to_string();
        let now = Utc::now();
        let now_str = now.to_rfc3339_opts(SecondsFormat::Millis, true);

        Mock::given(method("GET"))
            .and(path(format!(
                "/merchant-accounts/{}/transactions",
                merchant_account_id
            )))
            .and(query_param("from", &now_str))
            .and(query_param("to", &now_str))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": []
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let transactions = api
            .list_transactions(
                &merchant_account_id,
                &ListTransactionsRequest {
                    from: now,
                    to: now,
                    r#type: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(transactions, vec![]);
    }

    #[tokio::test]
    async fn list_transactions_not_found() {
        let (api, mock_server) = mock_client_and_server().await;

        let merchant_account_id = "merchant-account-id".to_string();
        let now = Utc::now();
        let now_str = now.to_rfc3339_opts(SecondsFormat::Millis, true);

        Mock::given(method("GET"))
            .and(path(format!(
                "/merchant-accounts/{}/transactions",
                merchant_account_id
            )))
            .and(query_param("from", &now_str))
            .and(query_param("to", &now_str))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&mock_server)
            .await;

        let res = api
            .list_transactions(
                &merchant_account_id,
                &ListTransactionsRequest {
                    from: now,
                    to: now,
                    r#type: None,
                },
            )
            .await;

        // Expect an error
        assert!(matches!(res, Err(Error::ApiError(e)) if e.status == 404));
    }
}
