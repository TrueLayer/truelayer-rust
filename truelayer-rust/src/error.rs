//! Standard errors used by all functions in the crate.

use futures::future::BoxFuture;
use std::{
    collections::HashMap,
    fmt,
    fmt::{Debug, Display, Formatter},
};

/// Error collecting all possible failures of the TrueLayer client.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Reqwest error.
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
    /// Error returned by a TrueLayer API endpoint.
    #[error("{0}")]
    ApiError(#[from] ApiError),
    /// Error during request signing.
    ///
    /// Read more about signing here: <https://docs.truelayer.com/docs/signing-your-requests>
    #[error("Error signing request: {0}")]
    SigningError(#[from] truelayer_signing::Error),
    /// Catch-all variant for unexpected errors.
    #[error(transparent)]
    Other(anyhow::Error),
}

impl From<reqwest_middleware::Error> for Error {
    fn from(e: reqwest_middleware::Error) -> Self {
        match e {
            reqwest_middleware::Error::Reqwest(e) => Error::HttpError(e),
            reqwest_middleware::Error::Middleware(e) => {
                e.downcast::<Error>().unwrap_or_else(Error::Other)
            }
        }
    }
}

impl From<Error> for reqwest_middleware::Error {
    fn from(e: Error) -> Self {
        reqwest_middleware::Error::Middleware(e.into())
    }
}

/// TrueLayer HTTP APIs error.
#[derive(thiserror::Error, Debug)]
pub struct ApiError {
    pub r#type: Option<String>,
    pub title: String,
    pub status: u16,
    pub trace_id: Option<String>,
    pub detail: Option<String>,
    pub errors: HashMap<String, Vec<String>>,
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TrueLayer HTTP error {}: {}", self.status, self.title)?;

        if let Some(ref r#type) = self.r#type {
            write!(f, " ({})", r#type)?;
        }

        if let Some(ref detail) = self.detail {
            write!(f, "\nAdditional details: {}", detail)?;
        }

        if let Some(ref trace_id) = self.trace_id {
            write!(f, "\nTrace ID: {}", trace_id)?;
        }

        if !self.errors.is_empty() {
            write!(f, "\nAll errors:")?;
            for (k, v) in &self.errors {
                write!(f, "\n- {}: {}", k, v.join(", "))?;
            }
        }

        Ok(())
    }
}

pub type AsyncBoxFn<Res> =
    Box<dyn FnMut() -> BoxFuture<'static, Result<Res, Error>> + Send + Sync + 'static>;

/// Wrapper around an [`Error`](crate::error::Error) that allows easy retrying
/// of the original request that caused the error.
pub struct RetryableError<Res> {
    error: Error,
    f: AsyncBoxFn<Res>,
}

impl<Res> RetryableError<Res> {
    pub(crate) async fn capture(mut f: AsyncBoxFn<Res>) -> Result<Res, Self> {
        f().await.map_err(|error| Self { error, f })
    }

    pub async fn retry(mut self) -> Result<Res, Self> {
        (*self.f)().await.map_err(|error| Self { error, ..self })
    }

    pub fn into_error(self) -> Error {
        self.into()
    }

    pub fn as_error(&self) -> &Error {
        &self.error
    }
}

impl<Res> Debug for RetryableError<Res> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.error, f)
    }
}

impl<Res> Display for RetryableError<Res> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.error, f)
    }
}

impl<Res> std::error::Error for RetryableError<Res> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.error.source()
    }
}

impl<Res> From<RetryableError<Res>> for Error {
    fn from(e: RetryableError<Res>) -> Self {
        e.error
    }
}

/// Internal helper to build a new `RetryableError`.
macro_rules! retryable {
    (| $($var:ident),* | $exp:expr) => {
        RetryableError::capture(Box::new(move || {
            $( let $var = $var.clone(); )*

            Box::pin(async move { $exp })
        }))
        .await
    };
}

pub(crate) use retryable;
