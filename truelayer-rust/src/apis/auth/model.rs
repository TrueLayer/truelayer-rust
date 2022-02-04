use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Credentials used to authenticate against TrueLayer's APIs.
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "grant_type", rename_all = "snake_case")]
pub enum Credentials {
    AuthorizationCode {
        client_id: String,
        client_secret: String,
        code: String,
        redirect_uri: String,
    },
    RefreshToken {
        client_id: String,
        client_secret: String,
        refresh_token: String,
    },
    ClientCredentials {
        client_id: String,
        client_secret: String,
        scope: String,
    },
}

impl Credentials {
    /// Returns a reference to the client id stored in this [`Credentials`](crate::apis::auth::Credentials).
    pub fn client_id(&self) -> &str {
        match self {
            Credentials::AuthorizationCode { client_id, .. }
            | Credentials::RefreshToken { client_id, .. }
            | Credentials::ClientCredentials { client_id, .. } => client_id,
        }
    }

    /// Returns a reference to the client secret stored in this [`Credentials`](crate::apis::auth::Credentials).
    pub fn client_secret(&self) -> &str {
        match self {
            Credentials::AuthorizationCode { client_secret, .. }
            | Credentials::RefreshToken { client_secret, .. }
            | Credentials::ClientCredentials { client_secret, .. } => client_secret,
        }
    }

    /// Returns a reference to the refresh token stored in this [`Credentials`](crate::apis::auth::Credentials).
    ///
    /// Returns `None` if the credential is not of type `RefreshToken`.
    pub fn refresh_token(&self) -> Option<&str> {
        match self {
            Credentials::RefreshToken { refresh_token, .. } => Some(refresh_token),
            _ => None,
        }
    }
}

/// Result of an authentication request.
#[derive(Clone, Debug)]
pub struct AuthenticationResult {
    pub(crate) access_token: AccessToken,
    pub(crate) refresh_token: Option<String>,
}

impl AuthenticationResult {
    /// Returns a reference to the [`AccessToken`](crate::apis::auth::AccessToken) returned by the authentication server.
    pub fn access_token(&self) -> &AccessToken {
        &self.access_token
    }

    /// Returns a reference to the refresh token returned by the authentication server, if present.
    pub fn refresh_token(&self) -> Option<&str> {
        self.refresh_token.as_deref()
    }
}

/// Opaque access token used to authenticate to TrueLayer APIs.
#[derive(Clone, Debug)]
pub struct AccessToken {
    pub(crate) token: String,
    pub(crate) expires_at: Option<DateTime<Utc>>,
}

impl AccessToken {
    /// Actual token held by this `Token` instance.
    pub fn token(&self) -> &str {
        &self.token
    }

    /// Expiration date of the token.
    ///
    /// Returns `None` if this token does not expire.
    pub fn expires_at(&self) -> Option<DateTime<Utc>> {
        self.expires_at
    }
}
