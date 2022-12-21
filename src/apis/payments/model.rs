use crate::{apis::auth::Token, pollable::IsInTerminalState, Error, Pollable, TrueLayerClient};
use anyhow::anyhow;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct CreatePaymentRequest {
    pub amount_in_minor: u64,
    pub currency: Currency,
    pub payment_method: PaymentMethodRequest,
    pub user: CreatePaymentUserRequest,
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PaymentMethodRequest {
    BankTransfer {
        provider_selection: ProviderSelectionRequest,
        beneficiary: Beneficiary,
    },
}

impl From<PaymentMethod> for PaymentMethodRequest {
    /// Builds a new payment method request configuration from an existing PaymentMethod
    fn from(p: PaymentMethod) -> Self {
        match p {
            PaymentMethod::BankTransfer {
                provider_selection,
                beneficiary,
            } => PaymentMethodRequest::BankTransfer {
                provider_selection: provider_selection.into(),
                beneficiary,
            },
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderSelectionRequest {
    UserSelected {
        filter: Option<ProviderFilter>,
        scheme_selection: Option<SchemeSelection>,
    },
    Preselected {
        provider_id: String,
        scheme_id: String,
        remitter: Option<Remitter>,
    },
}

impl From<ProviderSelection> for ProviderSelectionRequest {
    /// Builds a new ProviderSelectionRequest configuration from an existing ProviderSelection
    fn from(p: ProviderSelection) -> Self {
        match p {
            ProviderSelection::UserSelected {
                filter,
                scheme_selection,
                ..
            } => ProviderSelectionRequest::UserSelected {
                filter,
                scheme_selection,
            },
            ProviderSelection::Preselected {
                provider_id,
                scheme_id,
                remitter,
            } => ProviderSelectionRequest::Preselected {
                provider_id,
                scheme_id,
                remitter,
            },
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(untagged)]
pub enum CreatePaymentUserRequest {
    ExistingUser {
        id: String,
    },
    NewUser {
        name: Option<String>,
        email: Option<String>,
        phone: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreatePaymentResponse {
    pub id: String,
    pub resource_token: Token,
    pub user: CreatePaymentUserResponse,
    #[serde(flatten)]
    pub status: CreatePaymentStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CreatePaymentStatus {
    AuthorizationRequired,
    Authorized,
    Failed {
        failure_stage: FailureStage,
        failure_reason: String,
    },
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

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
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
    pub metadata: Option<HashMap<String, String>>,
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
        matches!(
            self.status,
            PaymentStatus::Executed { .. }
                | PaymentStatus::Settled { .. }
                | PaymentStatus::Failed { .. }
        )
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
        settlement_risk: Option<SettlementRisk>,
    },
    Settled {
        payment_source: PaymentSource,
        executed_at: DateTime<Utc>,
        settled_at: DateTime<Utc>,
        authorization_flow: Option<AuthorizationFlow>,
        settlement_risk: Option<SettlementRisk>,
    },
    Failed {
        failed_at: DateTime<Utc>,
        failure_stage: FailureStage,
        failure_reason: String,
        authorization_flow: Option<AuthorizationFlow>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(rename_all = "UPPERCASE")]
pub enum Currency {
    Eur,
    Gbp,
    Nok,
    Pln,
}

impl Display for Currency {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Currency::Eur => write!(f, "EUR"),
            Currency::Gbp => write!(f, "GBP"),
            Currency::Nok => write!(f, "NOK"),
            Currency::Pln => write!(f, "PLN"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum FailureStage {
    AuthorizationRequired,
    Authorizing,
    Authorized,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct PaymentSource {
    pub id: String,
    pub user_id: Option<String>,
    #[serde(default)]
    pub account_identifiers: Vec<AccountIdentifier>,
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
        account_holder_name: String,
        account_identifier: AccountIdentifier,
        reference: String,
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
pub struct SettlementRisk {
    pub category: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderSelection {
    UserSelected {
        filter: Option<ProviderFilter>,
        scheme_selection: Option<SchemeSelection>,
        provider_id: Option<String>,
        scheme_id: Option<String>,
    },
    Preselected {
        provider_id: String,
        scheme_id: String,
        remitter: Option<Remitter>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SchemeSelection {
    InstantOnly { allow_remitter_fee: Option<bool> },
    InstantPreferred { allow_remitter_fee: Option<bool> },
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct Remitter {
    pub account_holder_name: Option<String>,
    pub account_identifier: Option<AccountIdentifier>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct ProviderFilter {
    pub countries: Option<Vec<CountryCode>>,
    pub release_channel: Option<ReleaseChannel>,
    pub customer_segments: Option<Vec<CustomerSegment>>,
    pub provider_ids: Option<Vec<String>>,
    pub excludes: Option<ProviderFilterExcludes>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(rename_all = "UPPERCASE")]
pub enum CountryCode {
    AT,
    BE,
    DE,
    DK,
    ES,
    FI,
    FR,
    GB,
    IE,
    IT,
    LT,
    NL,
    NO,
    PL,
    PT,
    RO,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseChannel {
    GeneralAvailability,
    PublicBeta,
    PrivateBeta,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
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
    Consent {
        subsequent_action_hint: SubsequentAction,
    },
    Form {
        inputs: Vec<AdditionalInput>,
    },
    Wait,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SubsequentAction {
    Redirect,
    Form,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct Provider {
    pub id: String,
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
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AdditionalInput {
    Text {
        id: String,
        mandatory: bool,
        display_text: AdditionalInputDisplayText,
        description: Option<AdditionalInputDisplayText>,
        format: AdditionalInputFormat,
        sensitive: bool,
        min_length: i32,
        max_length: i32,
        regexes: Vec<AdditionalInputRegex>,
    },
    Select {
        id: String,
        mandatory: bool,
        display_text: AdditionalInputDisplayText,
        description: Option<AdditionalInputDisplayText>,
        options: Vec<AdditionalInputOption>,
    },
    TextWithImage {
        id: String,
        mandatory: bool,
        display_text: AdditionalInputDisplayText,
        description: Option<AdditionalInputDisplayText>,
        format: AdditionalInputFormat,
        sensitive: bool,
        min_length: i32,
        max_length: i32,
        regexes: Vec<AdditionalInputRegex>,
        image: AdditionalInputImage,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct AdditionalInputDisplayText {
    pub key: String,
    pub default: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AdditionalInputFormat {
    AccountNumber,
    Alphabetical,
    Alphanumerical,
    Any,
    Email,
    Iban,
    Numerical,
    SortCode,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct AdditionalInputRegex {
    pub regex: String,
    pub message: AdditionalInputDisplayText,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct AdditionalInputOption {
    pub id: String,
    pub display_text: AdditionalInputDisplayText,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AdditionalInputImage {
    Uri { uri: String },
    Base64 { data: String, media_type: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct AuthorizationFlowConfiguration {
    pub provider_selection: Option<ProviderSelectionSupported>,
    pub redirect: Option<RedirectSupported>,
    pub consent: Option<ConsentSupported>,
    pub form: Option<FormSupported>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct ProviderSelectionSupported {}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct RedirectSupported {
    pub return_uri: String,
    pub direct_return_uri: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct ConsentSupported {}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct FormSupported {
    pub input_types: Vec<AdditionalInputType>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AdditionalInputType {
    Text,
    Select,
    TextWithImage,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct User {
    pub id: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct StartAuthorizationFlowRequest {
    pub provider_selection: Option<ProviderSelectionSupported>,
    pub redirect: Option<RedirectSupported>,
    pub consent: Option<ConsentSupported>,
    pub form: Option<FormSupported>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct StartAuthorizationFlowResponse {
    pub authorization_flow: Option<AuthorizationFlow>,
    #[serde(flatten)]
    pub status: AuthorizationFlowResponseStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct SubmitProviderSelectionActionRequest {
    pub provider_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct SubmitProviderSelectionActionResponse {
    pub authorization_flow: Option<AuthorizationFlow>,
    #[serde(flatten)]
    pub status: AuthorizationFlowResponseStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct SubmitConsentActionResponse {
    pub authorization_flow: Option<AuthorizationFlow>,
    #[serde(flatten)]
    pub status: AuthorizationFlowResponseStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct SubmitFormActionRequest {
    pub inputs: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct SubmitFormActionResponse {
    pub authorization_flow: Option<AuthorizationFlow>,
    #[serde(flatten)]
    pub status: AuthorizationFlowResponseStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AuthorizationFlowResponseStatus {
    Authorizing,
    Failed {
        failure_stage: FailureStage,
        failure_reason: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct SubmitProviderReturnParametersRequest {
    pub query: String,
    pub fragment: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct SubmitProviderReturnParametersResponse {
    pub resource: SubmitProviderReturnParametersResponseResource,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SubmitProviderReturnParametersResponseResource {
    Payment { payment_id: String },
}

pub mod refunds {
    use std::collections::HashMap;

    use anyhow::anyhow;
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};

    use crate::{pollable::IsInTerminalState, Error, Pollable, TrueLayerClient};

    use super::Currency;

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
    pub struct CreateRefundRequest {
        pub amount_in_minor: Option<u64>,
        pub reference: String,
        pub metadata: Option<HashMap<String, String>>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
    pub struct CreateRefundResponse {
        pub id: String,
    }

    #[async_trait]
    impl Pollable for (&str, CreateRefundResponse) {
        type Output = Refund;

        async fn poll_once(&self, tl: &TrueLayerClient) -> Result<Self::Output, Error> {
            tl.payments
                .get_refund_by_id(self.0, &self.1.id)
                .await
                .transpose()
                .unwrap_or_else(|| Err(Error::Other(anyhow!("Refund returned 404 while polling"))))
        }
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
    pub struct Refund {
        pub id: String,
        pub amount_in_minor: u64,
        pub currency: Currency,
        pub reference: String,
        pub created_at: DateTime<Utc>,
        pub metadata: Option<HashMap<String, String>>,
        #[serde(flatten)]
        pub status: RefundStatus,
    }

    #[async_trait]
    impl Pollable for (&str, Refund) {
        type Output = Refund;

        async fn poll_once(&self, tl: &TrueLayerClient) -> Result<Self::Output, Error> {
            tl.payments
                .get_refund_by_id(self.0, &self.1.id)
                .await
                .transpose()
                .unwrap_or_else(|| Err(Error::Other(anyhow!("Refund returned 404 while polling"))))
        }
    }

    impl IsInTerminalState for Refund {
        /// A refund is considered to be in a terminal state if it is `Executed` or `Failed`.
        fn is_in_terminal_state(&self) -> bool {
            matches!(
                self.status,
                RefundStatus::Executed { .. } | RefundStatus::Failed { .. }
            )
        }
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
    #[serde(tag = "status", rename_all = "snake_case")]
    pub enum RefundStatus {
        Pending,
        Authorized,
        Executed {
            executed_at: DateTime<Utc>,
        },
        Failed {
            failed_at: DateTime<Utc>,
            failure_reason: String,
        },
    }
}
