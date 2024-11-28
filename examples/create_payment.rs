use anyhow::Context;
use truelayer_rust::{
    apis::{
        auth::Credentials,
        payments::{
            Beneficiary, CreatePaymentRequest, CreatePaymentUserRequest, Currency,
            PaymentMethodRequest, ProviderSelectionRequest,
        },
    },
    client::Environment,
    pollable::{PollOptions, PollableUntilTerminalState},
    TrueLayerClient,
};
use url::Url;

#[derive(serde::Deserialize, Debug)]
struct Config {
    client_id: String,
    client_secret: String,
    key_id: String,
    private_key: String,
    return_uri: Url,
}

impl Config {
    fn read() -> anyhow::Result<Self> {
        config::Config::builder()
            .add_source(config::File::with_name("config"))
            .build()?
            .try_deserialize()
            .context("Failed to assemble the required configuration")
    }
}

async fn run() -> anyhow::Result<()> {
    let config = Config::read()?;

    // Setup TrueLayer client
    let tl = TrueLayerClient::builder(Credentials::ClientCredentials {
        client_id: config.client_id,
        client_secret: config.client_secret.into(),
        scope: "payments".to_string(),
    })
    .with_signing_key(&config.key_id, config.private_key.into_bytes())
    .with_environment(Environment::Sandbox)
    .build();

    // List all merchant accounts
    let merchant_accounts = tl.merchant_accounts.list().await?;
    for merchant_account in &merchant_accounts {
        tracing::info!(
            "Merchant Account {}: Balance: {:.2} {}",
            merchant_account.id,
            merchant_account.available_balance_in_minor as f32 / 100.0,
            merchant_account.currency
        );
    }

    // Select the first one with GBP currency
    let merchant_account = merchant_accounts
        .into_iter()
        .find(|m| m.currency == Currency::Gbp)
        .context("Cannot find a GBP merchant account")?;

    // Create a new outgoing payment
    let res = tl
        .payments
        .create(&CreatePaymentRequest {
            amount_in_minor: 100,
            currency: Currency::Gbp,
            payment_method: PaymentMethodRequest::BankTransfer {
                provider_selection: ProviderSelectionRequest::UserSelected {
                    filter: None,
                    scheme_selection: None,
                },
                beneficiary: Beneficiary::MerchantAccount {
                    merchant_account_id: merchant_account.id,
                    account_holder_name: None,
                    reference: None,
                    statement_reference: None,
                },
            },
            user: CreatePaymentUserRequest::NewUser {
                name: Some("Some One".to_string()),
                email: Some("some.one@email.com".to_string()),
                phone: None,
            },
            metadata: None,
        })
        .await?;

    tracing::info!("Created new payment: {}", res.id);

    tracing::info!(
        "HPP Link: {}",
        tl.payments
            .get_hosted_payments_page_link(&res.id, &res.resource_token, config.return_uri.as_str())
            .await
    );

    tracing::info!("Begin waiting...");

    let completed_payment = res
        .poll_until_terminal_state(&tl, PollOptions::default())
        .await?;

    tracing::info!("{:#?}", completed_payment);

    Ok(())
}

#[tokio::main]
async fn main() {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Setting default subscriber failed");

    if let Err(e) = run().await {
        tracing::error!("Fatal error: {:?}", e);
        std::process::exit(1);
    }
}
