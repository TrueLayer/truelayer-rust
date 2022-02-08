use crate::apis::payments::{AccountIdentifier, Currency};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct MerchantAccount {
    pub id: String,
    pub currency: Currency,
    pub account_identifiers: Vec<AccountIdentifier>,
    pub available_balance_in_minor: u64,
    pub current_balance_in_minor: u64,
    pub account_holder_name: String,
}
