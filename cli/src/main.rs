mod log;

use anyhow::Context;
use truelayer_rust::{
    apis::{
        auth::Credentials,
        payments::{
            Beneficiary, CreatePaymentRequest, Currency, PaymentMethod, ProviderSelection, User,
        },
    },
    pollable::{PollOptions, PollableUntilTerminalState},
    TrueLayerClient,
};
use url::Url;
use uuid::Uuid;

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "UPPERCASE")]
struct Config {
    client_id: String,
    client_secret: String,
    auth_server_uri: Url,
    certificate_id: String,
    private_key: String,
    environment_uri: Url,
    return_uri: Url,
    hpp_uri: Url,
}

impl Config {
    fn read() -> anyhow::Result<Self> {
        let mut conf = config::Config::new();
        conf
            // Add in `./config.json`
            .merge(config::File::with_name("config"))?;
        conf.try_into()
            .context("Failed to assemble the required configuration")
    }
}

async fn run() -> anyhow::Result<()> {
    log::init();
    let config = Config::read()?;

    // Setup TrueLayer client
    let tl = TrueLayerClient::builder(Credentials::ClientCredentials {
        client_id: config.client_id,
        client_secret: config.client_secret,
        scope: "payments".to_string(),
    })
    .with_certificate(&config.certificate_id, config.private_key.into_bytes())
    .with_auth_url(config.auth_server_uri)
    .with_payments_url(config.environment_uri)
    .with_hosted_payments_page_url(config.hpp_uri)
    .build();

    // Create a new outgoing payment
    let res = tl
        .payments
        .create(
            &CreatePaymentRequest {
                amount_in_minor: 100,
                currency: Currency::Gbp,
                payment_method: PaymentMethod::BankTransfer {
                    provider_selection: ProviderSelection::UserSelected { filter: None },
                    beneficiary: Beneficiary::MerchantAccount {
                        merchant_account_id: "00000000-0000-0000-0000-000000000000".to_string(),
                        account_holder_name: None,
                    },
                },
                user: User {
                    id: Some(Uuid::new_v4().to_string()),
                    name: Some("Some One".to_string()),
                    email: Some("some.one@email.com".to_string()),
                    phone: None,
                },
            },
            &Uuid::new_v4().to_string(),
        )
        .await?;

    tracing::info!("Created new payment: {}", res.id);

    tracing::info!(
        "HPP Link: {}",
        tl.payments
            .get_hosted_payments_page_link(&res.id, &res.payment_token, config.return_uri.as_str())
            .await
    );

    tracing::info!("Begin waiting...");

    let completed_payment = res
        .poll_until_terminal_state(&tl, PollOptions::default())
        .await?;

    tracing::info!("{:?}", completed_payment);

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        tracing::error!("Fatal error: {:?}", e);
        std::process::exit(1);
    }
}
