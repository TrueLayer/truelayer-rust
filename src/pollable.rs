//! Common logic to poll for updates on resources.

use crate::{Error, TrueLayerClient};
use async_trait::async_trait;
use chrono::Utc;
use retry_policies::{policies::ExponentialBackoff, RetryDecision, RetryPolicy};
use std::time::Duration;

/// Options to configure the behaviour of [`Pollable::poll_until`](crate::pollable::Pollable::poll_until).
///
/// The default is an exponential backoff between retries from 1 to 30 seconds for a total of 5 minutes.
#[derive(Debug)]
pub struct PollOptions<R: RetryPolicy> {
    retry_policy: R,
}

impl Default for PollOptions<ExponentialBackoff> {
    fn default() -> Self {
        Self {
            retry_policy: ExponentialBackoff::builder()
                .retry_bounds(Duration::from_secs(1), Duration::from_secs(30))
                .build_with_total_retry_duration(Duration::from_secs(60 * 5 /* 5 mins */)),
        }
    }
}

impl<R: RetryPolicy> PollOptions<R> {
    /// Sets a retry policy.
    pub fn with_retry_policy<T: RetryPolicy>(self, retry_policy: T) -> PollOptions<T> {
        PollOptions { retry_policy }
    }
}

/// Error returned from [`Pollable::poll_until`](crate::pollable::Pollable::poll_until).
#[derive(thiserror::Error, Debug)]
pub enum PollError {
    /// Polling timed out before the condition was met.
    #[error("Polling timeout")]
    Timeout,
    /// Other error.
    #[error(transparent)]
    Error(#[from] Error),
}

/// A resource that can be continuously polled for updates.
#[async_trait]
pub trait Pollable: private::Sealed {
    type Output: Send;

    /// Makes a single request to retrieve the most up-to-date version of this resource from the server.
    async fn poll_once(&self, tl: &TrueLayerClient) -> Result<Self::Output, Error>;

    /// Continuously polls the server for updates on this resource until the given condition is met.
    #[tracing::instrument(name = "Poll for updates", skip_all)]
    async fn poll_until<R, F>(
        &self,
        tl: &TrueLayerClient,
        poll_options: PollOptions<R>,
        predicate: F,
    ) -> Result<Self::Output, PollError>
    where
        R: RetryPolicy + Send + Sync,
        F: for<'a> Fn(&'a Self::Output) -> bool + Send,
    {
        // Loop until we match the predicate
        let mut i = 0;
        loop {
            // Update the resource
            let res = self.poll_once(tl).await?;

            // Check predicate
            if predicate(&res) {
                return Ok(res);
            }

            // Wait
            match poll_options.retry_policy.should_retry(i) {
                RetryDecision::Retry { execute_after } => {
                    // Wait at least 1 second between each retry
                    let wait_time = Duration::from_secs(1)
                        .max((execute_after - Utc::now()).to_std().unwrap_or_default());

                    tracing::debug!(
                        "Waiting {} seconds before trying again",
                        wait_time.as_secs_f64()
                    );

                    tokio::time::sleep(wait_time).await;
                }
                RetryDecision::DoNotRetry => {
                    return Err(PollError::Timeout);
                }
            }

            i += 1;
        }
    }
}

/// A resource that can be in a terminal state.
pub trait IsInTerminalState {
    /// Returns `true` if this resource is in a terminal state.
    fn is_in_terminal_state(&self) -> bool;
}

/// A resource that can be continuously polled for updates until it reaches a terminal state.
#[async_trait]
pub trait PollableUntilTerminalState: Pollable {
    /// Continuously polls the server for updates on this resource until it reaches a terminal state.
    async fn poll_until_terminal_state<R: RetryPolicy + Send + Sync>(
        &self,
        tl: &TrueLayerClient,
        poll_options: PollOptions<R>,
    ) -> Result<Self::Output, PollError>;
}

#[async_trait]
impl<T> PollableUntilTerminalState for T
where
    T: Pollable + Send + Sync,
    <T as Pollable>::Output: IsInTerminalState,
{
    async fn poll_until_terminal_state<R: RetryPolicy + Send + Sync>(
        &self,
        tl: &TrueLayerClient,
        poll_options: PollOptions<R>,
    ) -> Result<Self::Output, PollError> {
        self.poll_until(tl, poll_options, Self::Output::is_in_terminal_state)
            .await
    }
}

// Prevent users from implementing the `Pollable` trait.
mod private {
    pub trait Sealed {}

    impl Sealed for crate::apis::payments::Payment {}
    impl Sealed for crate::apis::payments::CreatePaymentResponse {}

    #[cfg(test)]
    impl<F> Sealed for super::tests::PollableMock<F> {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{apis::auth::Credentials, client::Environment};
    use anyhow::anyhow;
    use reqwest::Url;
    use std::{
        sync::{
            atomic::{AtomicU32, Ordering},
            Arc, Mutex,
        },
        time::Instant,
    };

    /// Mock for `Pollable` tests.
    pub struct PollableMock<F> {
        f: Arc<Mutex<F>>,
        polled_count: Arc<AtomicU32>,
        terminal_state_after: u32,
    }

    impl<F> PollableMock<F> {
        fn new(f: F) -> Self {
            Self {
                f: Arc::new(Mutex::new(f)),
                polled_count: Arc::new(AtomicU32::new(0)),
                terminal_state_after: u32::MAX,
            }
        }

        fn with_terminal_state_after(mut self, terminal_state_after: u32) -> Self {
            self.terminal_state_after = terminal_state_after;
            self
        }

        fn polled_count(&self) -> u32 {
            self.polled_count.load(Ordering::SeqCst)
        }
    }

    impl<F> Clone for PollableMock<F> {
        fn clone(&self) -> Self {
            Self {
                f: self.f.clone(),
                polled_count: self.polled_count.clone(),
                terminal_state_after: self.terminal_state_after,
            }
        }
    }

    impl<F> IsInTerminalState for PollableMock<F> {
        fn is_in_terminal_state(&self) -> bool {
            self.polled_count() >= self.terminal_state_after
        }
    }

    #[async_trait]
    impl<F> Pollable for PollableMock<F>
    where
        F: FnMut(u32) -> Option<Error> + Send + Sync,
    {
        type Output = PollableMock<F>;

        async fn poll_once(&self, _tl: &TrueLayerClient) -> Result<Self::Output, Error> {
            self.polled_count.fetch_add(1, Ordering::SeqCst);

            match (self.f.lock().unwrap())(self.polled_count()) {
                None => Ok(self.clone()),
                Some(e) => Err(e),
            }
        }
    }

    fn mock_tl_client() -> TrueLayerClient {
        TrueLayerClient::builder(Credentials::ClientCredentials {
            client_id: "".to_string(),
            client_secret: "".to_string().into(),
            scope: "".to_string(),
        })
        .with_environment(Environment::from_single_url(
            &Url::parse("https://non.existent.domain").unwrap(),
        ))
        .build()
    }

    #[tokio::test]
    async fn poll_until() {
        let pollable = PollableMock::new(|_| None);

        // This will poll three times, and the third time the predicate will match.
        let start = Instant::now();
        pollable
            .poll_until(&mock_tl_client(), PollOptions::default(), |_| {
                pollable.polled_count() >= 3
            })
            .await
            .unwrap();

        // Assert that at we waited at least two seconds, which is the minimum wait time between retries * 2
        let elapsed = Instant::now() - start;
        assert_eq!(pollable.polled_count(), 3);
        assert!(elapsed >= Duration::from_secs(2));
    }

    #[tokio::test]
    async fn poll_until_timeout() {
        let pollable = PollableMock::new(|_| None);

        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(Duration::from_secs(1), Duration::from_secs(1))
            .build_with_max_retries(2);

        // This should poll forever, but the retry policy will timeout after two retries
        let start = Instant::now();
        let res = pollable
            .poll_until(
                &mock_tl_client(),
                PollOptions::default().with_retry_policy(retry_policy),
                |_| false,
            )
            .await;
        let elapsed = Instant::now() - start;

        // Assert we got a timeout error
        assert!(matches!(res, Err(PollError::Timeout)));

        // Assert that at we waited at least two seconds, which is the minimum wait time between retries * 2
        assert_eq!(pollable.polled_count(), 3);
        assert!(elapsed >= Duration::from_secs(2));
    }

    #[tokio::test]
    async fn poll_until_error() {
        let pollable = PollableMock::new(|polled_count| {
            if polled_count >= 2 {
                Some(Error::Other(anyhow!("Test error")))
            } else {
                None
            }
        });

        // This should poll forever, but the resource will return an error at the second retry
        let start = Instant::now();
        let res = pollable
            .poll_until(&mock_tl_client(), PollOptions::default(), |_| false)
            .await;
        let elapsed = Instant::now() - start;

        // Assert we got an error
        assert!(matches!(res, Err(PollError::Error(Error::Other(_)))));

        // Assert that at we waited at least one second, which is the minimum wait time between retries
        assert_eq!(pollable.polled_count(), 2);
        assert!(elapsed >= Duration::from_secs(1));
    }

    #[tokio::test]
    async fn poll_until_terminal_state() {
        let pollable = PollableMock::new(|_| None).with_terminal_state_after(2);

        // This will poll three times, and the third time the predicate will match.
        let start = Instant::now();
        pollable
            .poll_until_terminal_state(&mock_tl_client(), PollOptions::default())
            .await
            .unwrap();

        // Assert that at we waited at least one second, which is the minimum wait time between retries
        let elapsed = Instant::now() - start;
        assert_eq!(pollable.polled_count(), 2);
        assert!(pollable.is_in_terminal_state());
        assert!(elapsed >= Duration::from_secs(1));
    }
}
