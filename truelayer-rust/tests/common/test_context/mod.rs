#[cfg(not(feature = "acceptance-tests"))]
mod local_mock;
#[cfg(feature = "acceptance-tests")]
mod sandbox;

#[cfg(not(feature = "acceptance-tests"))]
pub use local_mock::TestContext;
#[cfg(feature = "acceptance-tests")]
pub use sandbox::TestContext;
