use crate::apis::{
    payments::{AccountIdentifier, Currency, ExternalPaymentRemitter, PaymentSource},
    payouts::PayoutBeneficiary,
};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize, Serializer};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct MerchantAccount {
    pub id: String,
    pub currency: Currency,
    pub account_identifiers: Vec<AccountIdentifier>,
    pub available_balance_in_minor: u64,
    pub current_balance_in_minor: u64,
    pub account_holder_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct SetupSweepingRequest {
    pub max_amount_in_minor: u64,
    pub currency: Currency,
    pub frequency: SweepingFrequency,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SweepingFrequency {
    Daily,
    Weekly,
    Fortnightly,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct SweepingSettings {
    pub max_amount_in_minor: u64,
    pub currency: Currency,
    pub frequency: SweepingFrequency,
    pub destination: AccountIdentifier,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct ListPaymentSourcesRequest {
    pub user_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct ListTransactionsRequest {
    #[serde(serialize_with = "serialize_timestamp")]
    pub from: DateTime<Utc>,
    #[serde(serialize_with = "serialize_timestamp")]
    pub to: DateTime<Utc>,
    pub r#type: Option<TransactionTypeFilter>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TransactionTypeFilter {
    Payment,
    Payout,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct Transaction {
    pub id: String,
    pub currency: Currency,
    pub amount_in_minor: u64,
    #[serde(flatten)]
    pub r#type: TransactionType,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransactionType {
    MerchantAccountPayment {
        status: TransactionPayinStatus,
        settled_at: DateTime<Utc>,
        payment_source: PaymentSource,
        payment_id: String,
    },
    ExternalPayment {
        status: TransactionPayinStatus,
        settled_at: DateTime<Utc>,
        remitter: ExternalPaymentRemitter,
    },
    Payout {
        #[serde(flatten)]
        status: TransactionPayoutStatus,
        created_at: DateTime<Utc>,
        beneficiary: PayoutBeneficiary,
        context_code: TransactionPayoutContextCode,
        payout_id: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TransactionPayinStatus {
    Settled,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TransactionPayoutStatus {
    Pending,
    Executed { executed_at: DateTime<Utc> },
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TransactionPayoutContextCode {
    Withdrawal,
    ServicePayment,
    Internal,
}

fn serialize_timestamp<S>(timestamp: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&timestamp.to_rfc3339_opts(SecondsFormat::Millis, true))
}
