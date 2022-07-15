use serde::{Deserialize, Serialize};

use crate::apis::payments::CountryCode;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct Provider {
    pub id: String,
    pub display_name: Option<String>,
    pub icon_uri: Option<String>,
    pub logo_uri: Option<String>,
    pub bg_color: Option<String>,
    pub country_code: Option<CountryCode>,
    pub capabilities: Capabilities,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct Capabilities {
    pub payments: capabilities::Payments,
}

pub mod capabilities {
    use serde::{Deserialize, Serialize};

    use crate::apis::payments::ReleaseChannel;

    use super::PaymentScheme;

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
    pub struct Payments {
        pub bank_transfer: Option<BankTransfer>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
    pub struct BankTransfer {
        pub release_channel: ReleaseChannel,
        pub schemes: Vec<PaymentScheme>,
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct PaymentScheme {
    pub id: String,
}
