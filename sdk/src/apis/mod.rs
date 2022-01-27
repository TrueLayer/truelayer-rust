use crate::authenticator::Authenticator;
use reqwest::Url;
use reqwest_middleware::ClientWithMiddleware;
use std::fmt::{Debug, Formatter};

pub mod auth;
pub mod payments;

pub(crate) struct TrueLayerClientInner {
    pub(crate) client: ClientWithMiddleware,
    pub(crate) authenticator: Authenticator,
    pub(crate) payments_url: Url,
    pub(crate) hpp_url: Url,
}

impl Debug for TrueLayerClientInner {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrueLayerClientInner")
            .finish_non_exhaustive()
    }
}
