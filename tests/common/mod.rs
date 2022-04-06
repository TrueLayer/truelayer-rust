use std::{
    future::Future,
    time::{Duration, Instant},
};

#[cfg(not(feature = "acceptance-tests"))]
mod mock_server;
pub mod test_context;

#[derive(serde::Serialize, Clone, Debug, Eq, PartialEq)]
pub enum MockBankAction {
    Execute,
    RejectAuthorisation,
    RejectExecution,
    Cancel,
}

/// Retries the given asynchronous function until it returns `Some(_)` or times out.
pub async fn retry<F, O, T>(max_wait: Duration, f: F) -> Option<T>
where
    F: Fn() -> O,
    O: Future<Output = Option<T>>,
{
    let start = Instant::now();
    loop {
        match f().await {
            Some(out) => return Some(out),
            None => {
                if start.elapsed() < max_wait {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                } else {
                    return None;
                }
            }
        };
    }
}
