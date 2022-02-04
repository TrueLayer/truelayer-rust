use crate::{pollable::IsInTerminalState, Error, Pollable, TrueLayerClient};
use anyhow::anyhow;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreatePaymentRequest {
    pub amount_in_minor: u64,
    pub currency: Currency,
    pub payment_method: PaymentMethod,
    pub user: User,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreatePaymentResponse {
    pub id: String,
    pub payment_token: String,
    pub user: CreatePaymentUserResponse,
}

#[async_trait]
impl Pollable for CreatePaymentResponse {
    type Output = Payment;

    async fn poll_once(&self, tl: &TrueLayerClient) -> Result<Self::Output, Error> {
        tl.payments
            .get_by_id(&self.id)
            .await
            .transpose()
            .unwrap_or_else(|| Err(Error::Other(anyhow!("Payment returned 404 while polling"))))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreatePaymentUserResponse {
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct Payment {
    pub id: String,
    pub amount_in_minor: u64,
    pub currency: Currency,
    pub user: User,
    pub payment_method: PaymentMethod,
    pub created_at: DateTime<Utc>,
    #[serde(flatten)]
    pub status: PaymentStatus,
}

#[async_trait]
impl Pollable for Payment {
    type Output = Payment;

    async fn poll_once(&self, tl: &TrueLayerClient) -> Result<Self::Output, Error> {
        tl.payments
            .get_by_id(&self.id)
            .await
            .transpose()
            .unwrap_or_else(|| Err(Error::Other(anyhow!("Payment returned 404 while polling"))))
    }
}

impl IsInTerminalState for Payment {
    /// A payment is considered to be in a terminal state if it is `Executed`, `Settled` or `Failed`.
    fn is_in_terminal_state(&self) -> bool {
        match self.status {
            PaymentStatus::AuthorizationRequired { .. }
            | PaymentStatus::Authorizing { .. }
            | PaymentStatus::Authorized { .. } => false,
            PaymentStatus::Executed { .. }
            | PaymentStatus::Settled { .. }
            | PaymentStatus::Failed { .. } => true,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PaymentStatus {
    AuthorizationRequired,
    Authorizing {
        authorization_flow: AuthorizationFlow,
    },
    Authorized {
        authorization_flow: Option<AuthorizationFlow>,
    },
    Executed {
        executed_at: DateTime<Utc>,
        authorization_flow: Option<AuthorizationFlow>,
    },
    Settled {
        payment_source: PaymentSource,
        executed_at: DateTime<Utc>,
        settled_at: DateTime<Utc>,
        authorization_flow: Option<AuthorizationFlow>,
    },
    Failed {
        failed_at: DateTime<Utc>,
        failure_stage: FailureStage,
        failure_reason: String,
        authorization_flow: Option<AuthorizationFlow>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum Currency {
    Gbp,
    Eur,
}

impl Display for Currency {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Currency::Gbp => write!(f, "GBP"),
            Currency::Eur => write!(f, "EUR"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FailureStage {
    AuthorizationRequired,
    Authorizing,
    Authorized,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct PaymentSource {
    pub id: String,
    pub account_identifiers: Option<Vec<AccountIdentifier>>,
    pub account_holder_name: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PaymentMethod {
    BankTransfer {
        provider_selection: ProviderSelection,
        beneficiary: Beneficiary,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Beneficiary {
    MerchantAccount {
        merchant_account_id: String,
        account_holder_name: Option<String>,
    },
    ExternalAccount {
        account_holder_name: Option<String>,
        reference: String,
        account_identifier: AccountIdentifier,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AccountIdentifier {
    SortCodeAccountNumber {
        sort_code: String,
        account_number: String,
    },
    Iban {
        iban: String,
    },
    Bban {
        bban: String,
    },
    Nrb {
        nrb: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderSelection {
    UserSelected {
        filter: Option<ProviderFilter>,
    },
    Preselected {
        provider_id: String,
        scheme_id: String,
        remitter: Option<Remitter>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct Remitter {
    account_holder_name: Option<String>,
    account_identifier: Option<AccountIdentifier>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct ProviderFilter {
    pub countries: Option<Vec<CountryCode>>,
    pub release_channel: Option<ReleaseChannel>,
    pub customer_segments: Option<Vec<CustomerSegment>>,
    pub provider_ids: Option<Vec<String>>,
    pub excludes: Option<ProviderFilterExcludes>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum CountryCode {
    GB,
    FR,
    IE,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseChannel {
    GeneralAvailability,
    PublicBeta,
    PrivateBeta,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CustomerSegment {
    Retail,
    Business,
    Corporate,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct ProviderFilterExcludes {
    pub provider_ids: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct AuthorizationFlow {
    pub actions: Option<AuthorizationFlowActions>,
    pub configuration: Option<AuthorizationFlowConfiguration>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct AuthorizationFlowActions {
    pub next: AuthorizationFlowNextAction,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthorizationFlowNextAction {
    ProviderSelection {
        providers: Vec<Provider>,
    },
    Redirect {
        uri: String,
        metadata: Option<RedirectActionMetadata>,
    },
    Wait,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct Provider {
    pub provider_id: Option<String>,
    pub display_name: Option<String>,
    pub icon_uri: Option<String>,
    pub logo_uri: Option<String>,
    pub bg_color: Option<String>,
    pub country_code: Option<CountryCode>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RedirectActionMetadata {
    Provider(Provider),
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct AuthorizationFlowConfiguration {
    pub provider_selection: ProviderSelectionSupported,
    pub redirect: RedirectSupported,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ProviderSelectionSupported {
    NotSupported,
    Supported,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RedirectSupported {
    NotSupported,
    Supported { return_uri: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct User {
    pub id: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}
