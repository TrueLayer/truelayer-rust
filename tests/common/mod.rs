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
