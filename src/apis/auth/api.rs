use crate::{
    apis::{auth::AuthenticationResult, TrueLayerClientInner},
    Error,
};
use std::sync::Arc;

/// TrueLayer authentication API client.
#[derive(Debug, Clone)]
pub struct AuthApi {
    inner: Arc<TrueLayerClientInner>,
}

impl AuthApi {
    pub(crate) fn new(inner: Arc<TrueLayerClientInner>) -> Self {
        Self { inner }
    }

    /// Returns the current [`AccessToken`](crate::apis::auth::AccessToken) used to authenticate to the TrueLayer APIs.
    /// If the client is not authenticated yet, a new authentication request
    /// using the configured credentials will be fired.
    pub async fn get_access_token(&self) -> Result<AuthenticationResult, Error> {
        // Just delegate to the authenticator
        self.inner.authenticator.get_access_token().await
    }
}
