//! Clients for the various TrueLayer APIs.

use crate::{authenticator::Authenticator, client::Environment};
use reqwest_middleware::ClientWithMiddleware;
use std::fmt::{Debug, Formatter};

pub mod auth;
pub mod merchant_accounts;
pub mod payments;
pub mod payments_providers;
pub mod payouts;
pub mod stablecoin;

pub(crate) struct TrueLayerClientInner {
    pub(crate) client: ClientWithMiddleware,
    pub(crate) authenticator: Authenticator,
    pub(crate) environment: Environment,
}

impl Debug for TrueLayerClientInner {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrueLayerClientInner")
            .field("environment", &self.environment)
            .finish_non_exhaustive()
    }
}
