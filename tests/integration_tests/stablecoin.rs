use crate::common::test_context::TestContext;

#[tokio::test]
async fn valid_test_signature_success() {
    let ctx = TestContext::start().await;

    let res = ctx.client.stablecoin.test_signature().await;

    assert!(res.is_ok());
}
