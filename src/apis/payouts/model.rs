use crate::{
    apis::payments::{AccountIdentifier, Currency},
    pollable::IsInTerminalState,
    Error, Pollable, TrueLayerClient,
};
use anyhow::anyhow;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use derive_builder::Builder;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Builder)]
#[cfg_attr(not(feature = "compat"), non_exhaustive)]
pub struct CreatePayoutRequest {
    pub merchant_account_id: String,
    pub amount_in_minor: u64,
    pub currency: Currency,
    pub beneficiary: PayoutBeneficiary,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreatePayoutResponse {
    pub id: String,
}

#[async_trait]
impl Pollable for CreatePayoutResponse {
    type Output = Payout;

    async fn poll_once(&self, tl: &TrueLayerClient) -> Result<Self::Output, Error> {
        tl.payouts
            .get_by_id(&self.id)
            .await
            .transpose()
            .unwrap_or_else(|| Err(Error::Other(anyhow!("Payout returned 404 while polling"))))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(not(feature = "compat"), non_exhaustive)]
pub enum PayoutBeneficiary {
    ExternalAccount(ExternalAccount),
    PaymentSource(PaymentSource),
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Builder)]
pub struct ExternalAccount {
    pub account_holder_name: String,
    pub account_identifier: AccountIdentifier,
    pub reference: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Builder)]
pub struct PaymentSource {
    pub user_id: String,
    pub payment_source_id: String,
    pub reference: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Payout {
    pub id: String,
    pub merchant_account_id: String,
    pub amount_in_minor: u64,
    pub currency: Currency,
    pub beneficiary: PayoutBeneficiary,
    pub created_at: DateTime<Utc>,
    #[serde(flatten)]
    pub status: PayoutStatus,
}

#[async_trait]
impl Pollable for Payout {
    type Output = Payout;

    async fn poll_once(&self, tl: &TrueLayerClient) -> Result<Self::Output, Error> {
        tl.payouts
            .get_by_id(&self.id)
            .await
            .transpose()
            .unwrap_or_else(|| Err(Error::Other(anyhow!("Payout returned 404 while polling"))))
    }
}

impl IsInTerminalState for Payout {
    /// A payout is considered to be in a terminal state if it is `Executed` or `Failed`.
    fn is_in_terminal_state(&self) -> bool {
        matches!(
            self.status,
            PayoutStatus::Executed { .. } | PayoutStatus::Failed { .. }
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
#[cfg_attr(not(feature = "compat"), non_exhaustive)]
pub enum PayoutStatus {
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
