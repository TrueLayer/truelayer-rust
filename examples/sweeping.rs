use anyhow::Context;
use dialoguer::{console::style, theme::ColorfulTheme, Confirm, Input, Select};
use truelayer_rust::{
    apis::{
        auth::Credentials,
        merchant_accounts::{SetupSweepingRequest, SweepingFrequency},
    },
    client::Environment,
    TrueLayerClient,
};

#[derive(serde::Deserialize, Debug)]
struct Config {
    client_id: String,
    client_secret: String,
    key_id: String,
    private_key: String,
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

    // Let the user select one account
    let merchant_account_index = dialoguer::Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a merchant account")
        .items(
            &merchant_accounts
                .iter()
                .map(|m| {
                    format!(
                        "Account {} (available balance {:.2} {})",
                        m.id,
                        m.available_balance_in_minor as f64 / 100.0,
                        m.currency
                    )
                })
                .collect::<Vec<_>>(),
        )
        .default(0)
        .interact()?;
    let merchant_account = &merchant_accounts[merchant_account_index];

    // Get current sweeping settings
    let sweeping_settings = tl
        .merchant_accounts
        .get_sweeping_settings(&merchant_account.id)
        .await?;

    let dot = style("·".to_string()).for_stderr().black().bright();
    println!(
        "{} Current sweeping settings: {:?}",
        dot,
        style(sweeping_settings).bold().cyan()
    );

    let enable = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Do you want to enable sweeping for this account?")
        .default(true)
        .interact()?;

    if enable {
        let amount: u64 = Input::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "Sweeping threshold (in minor of {})",
                merchant_account.currency
            ))
            .with_initial_text("100")
            .interact_text()?;
        let frequency = &[
            SweepingFrequency::Daily,
            SweepingFrequency::Weekly,
            SweepingFrequency::Fortnightly,
        ][Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Sweeping frequency")
            .items(&["Daily", "Weekly", "Fortnightly"])
            .default(0)
            .interact()?];

        // Update sweeping settings
        tl.merchant_accounts
            .setup_sweeping(
                &merchant_account.id,
                &SetupSweepingRequest {
                    max_amount_in_minor: amount,
                    currency: merchant_account.currency,
                    frequency: frequency.clone(),
                },
            )
            .await?;
        println!(
            "{} {}",
            dot,
            style("Sweeping settings updated!").bold().green()
        );
    } else {
        // Disable sweeping
        tl.merchant_accounts
            .disable_sweeping(&merchant_account.id)
            .await?;
        println!("{} {}", dot, style("Sweeping disabled!").bold().red());
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Fatal error: {:?}", e);
        std::process::exit(1);
    }
}
