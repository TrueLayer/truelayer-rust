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
    /// Error building request signature.
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
    /// A unique identifier for this class of error.
    ///
    /// It's typically a URL pointing to a webpage with more information on the error.
    pub r#type: String,
    /// Concise description of the error.
    pub title: String,
    /// HTTP status returned by the server.
    pub status: u16,
    /// The TrueLayer trace identifier for the request.
    pub trace_id: Option<String>,
    /// A human readable explanation specific to this occurrence of the problem.
    pub detail: Option<String>,
    /// Optional additional details depending on the specific error.
    ///
    /// In the case of validation errors, this map contains a list of all the fields that failed validation.
    pub errors: HashMap<String, Vec<String>>,
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TrueLayer HTTP error {}: {} ({})",
            self.status, self.title, self.r#type
        )?;

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
