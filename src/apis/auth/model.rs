use chrono::{DateTime, Utc};
use secrecy::{ExposeSecret, Secret};
use serde::{Deserialize, Serialize};
use std::ops::Deref;

/// Credentials used to authenticate against TrueLayer's APIs.
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "grant_type", rename_all = "snake_case")]
pub enum Credentials {
    AuthorizationCode {
        client_id: String,
        client_secret: Token,
        code: String,
        redirect_uri: String,
    },
    RefreshToken {
        client_id: String,
        client_secret: Token,
        refresh_token: Token,
    },
    ClientCredentials {
        client_id: String,
        client_secret: Token,
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
    pub fn client_secret(&self) -> &Token {
        match self {
            Credentials::AuthorizationCode { client_secret, .. }
            | Credentials::RefreshToken { client_secret, .. }
            | Credentials::ClientCredentials { client_secret, .. } => client_secret,
        }
    }

    /// Returns a reference to the refresh token stored in this [`Credentials`](crate::apis::auth::Credentials).
    ///
    /// Returns `None` if the credential is not of type `RefreshToken`.
    pub fn refresh_token(&self) -> Option<&Token> {
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
    pub(crate) refresh_token: Option<Token>,
}

impl AuthenticationResult {
    /// Returns a reference to the [`AccessToken`](crate::apis::auth::AccessToken) returned by the authentication server.
    pub fn access_token(&self) -> &AccessToken {
        &self.access_token
    }

    /// Returns a reference to the refresh token returned by the authentication server, if present.
    pub fn refresh_token(&self) -> Option<&Token> {
        self.refresh_token.as_ref()
    }
}

/// Opaque access token used to authenticate to TrueLayer APIs.
#[derive(Clone, Debug)]
pub struct AccessToken {
    pub(crate) token: Token,
    pub(crate) expires_at: Option<DateTime<Utc>>,
}

impl AccessToken {
    /// Actual token contents held by this `AccessToken` instance.
    pub fn token(&self) -> &Token {
        &self.token
    }

    /// Expiration date of the token.
    ///
    /// Returns `None` if this token does not expire.
    pub fn expires_at(&self) -> Option<DateTime<Utc>> {
        self.expires_at
    }
}

impl Deref for AccessToken {
    type Target = Token;

    fn deref(&self) -> &Self::Target {
        self.token()
    }
}

/// Wrapper for a secret string that makes it harder to accidentally expose secrets
/// and ensures the backing memory is wiped on drop.
///
/// It is a wrapper around a [`secrecy::Secret`](secrecy::Secret).
///
/// ```rust
/// # use truelayer_rust::apis::auth::Token;
/// let token = Token::new("supersecret");
///
/// // The secret is redacted when printed with Debug
/// assert!(!format!("{:?}", token).contains("supersecret"));
///
/// // But can be manually exposed calling `expose_secret()`...
/// assert_eq!(token.expose_secret(), "supersecret");
///
/// // ... Or if serialized with Serde
/// let serialized = serde_json::to_string(&token).unwrap();
/// assert!(serialized.contains("supersecret"));
/// ```
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Token(#[serde(serialize_with = "serialize_secret")] Secret<String>);

impl Token {
    /// Wraps a secret string in a new `Token`.
    pub fn new<T: Into<String>>(s: T) -> Self {
        Self(Secret::new(s.into()))
    }

    /// Exposes a reference to the underlying secret string.
    pub fn expose_secret(&self) -> &str {
        self.0.expose_secret()
    }
}

impl<T> From<T> for Token
where
    T: Into<String>,
{
    fn from(s: T) -> Self {
        Token::new(s)
    }
}

fn serialize_secret<S>(secret: &Secret<String>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::ser::Serializer,
{
    secret.expose_secret().serialize(serializer)
}
