use crate::common::test_context::TestContext;
use truelayer_rust::{apis::auth::Credentials, error::ApiError, Error, TrueLayerClient};

#[tokio::test]
async fn get_access_token() {
    let ctx = TestContext::start().await;

    let access_token = ctx
        .client
        .auth
        .get_access_token()
        .await
        .unwrap()
        .access_token()
        .clone();

    assert!(!access_token.expose_secret().is_empty());
}

#[tokio::test]
async fn invalid_credentials() {
    let ctx = TestContext::start().await;

    // Create a new client with a set of invalid credentials pointing to the same mock server
    let client = TrueLayerClient::builder(Credentials::ClientCredentials {
        client_id: "invalid".to_string(),
        client_secret: "invalid".to_string().into(),
        scope: "payments paydirect".to_string(),
    })
    .with_environment(ctx.tl_environment())
    .build();

    // Make the request and assert that we got an error
    let err = client
        .auth
        .get_access_token()
        .await
        .expect_err("Expected error");
    assert!(matches!(err, Error::ApiError(ApiError { title, .. }) if title == "invalid_client"));
}
