//! Standard errors used by all functions in the crate.

use std::{collections::HashMap, fmt};

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
    pub errors: Option<HashMap<String, Vec<String>>>,
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

        if let Some(ref errors) = self.errors {
            write!(f, "\nAll errors:")?;
            for (k, v) in errors {
                write!(f, "\n- {}: {}", k, v.join(", "))?;
            }
        }

        Ok(())
    }
}
