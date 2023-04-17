use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct ListOnRampResponse {
    pub items: Vec<OnRamp>,
    pub pagination: Pagination,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Pagination {
    pub next_cursor: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct OnRamp {
    pub id: String,
    pub status: OnRampStatus,
    pub status_detail: Option<String>,
    pub fiat_currency: FiatCurrency,
    pub blockchain_network: BlockchainNetwork,
    pub stablecoin_token: StablecoinToken,
    pub stablecoin_destination_address: String,
    pub fiat_amount_in_minor: i64,
    pub fiat_fee_in_minor: Option<i64>,
    pub price_plan_id: Option<String>,
    pub price_plan_name: Option<String>,
    pub stablecoin_token_amount_in_minor: Option<i64>,
    pub blockchain_transaction_hash: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnRampStatus {
    /// On-ramp transaction is pending.
    Pending,
    /// On-ramp transaction is completed.
    Completed,
    /// On-ramp transaction is failed.
    Failed,
}

#[derive(Debug, Clone, Deserialize)]
pub enum FiatCurrency {
    GBP,
}

#[derive(Debug, Clone, Deserialize)]
pub enum StablecoinToken {
    GBPL,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockchainNetwork {
    Ethereum,
    Polygon,
}
