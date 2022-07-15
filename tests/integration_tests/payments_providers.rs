use crate::common::test_context::TestContext;
use truelayer_rust::apis::{
    payments::{CountryCode, ReleaseChannel},
    payments_providers::{capabilities, Capabilities, PaymentScheme},
};

#[tokio::test]
async fn get_by_id_successful() {
    let ctx = TestContext::start().await;

    let provider_id = "mock-payments-gb-redirect";

    // Retrieve the details of the same merchant account we use to test payments
    let provider = ctx
        .client
        .payments_providers
        .get_by_id(provider_id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(provider.id, provider_id);
    assert_eq!(
        provider.display_name,
        Some("Mock UK Payments - Redirect Flow".into())
    );
    assert_eq!(provider.country_code, Some(CountryCode::GB));
    assert_eq!(
        provider.capabilities,
        Capabilities {
            payments: capabilities::Payments {
                bank_transfer: Some(capabilities::BankTransfer {
                    release_channel: ReleaseChannel::GeneralAvailability,
                    schemes: vec![PaymentScheme {
                        id: "faster_payments_service".into()
                    },]
                })
            }
        }
    );
}
