mod log;

use std::str::FromStr;
use std::time::Duration;

use anyhow::Context;
use sdk::auth::Client;
use sdk::create_payment::{NewUserInfo, Payment, Secrets, User};
use sdk::TlBuilder;
use url::Url;
use uuid::Uuid;

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "UPPERCASE")]
struct Config {
    client_id: String,
    client_secret: Uuid,
    auth_server_uri: Url,
    certificate_id: Uuid,
    private_key: String,
    environment_uri: Url,
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

fn payment() -> Payment {
    Payment {
        amount_in_minor: 1,
        currency: sdk::create_payment::Currency::Gbp,
        payment_method: sdk::create_payment::PaymentMethod::BankTransfer,
        beneficiary: sdk::create_payment::Beneficiary::MerchantAccount {
            // TODO get merchant id automatically
            id: Uuid::from_str("7cb988f6-81e9-4788-b4bd-f7252468822c").unwrap(),
            name: "First Last".to_owned(),
        },
        user: User::New {
            name: "username".to_string(),
            info: NewUserInfo::with_email("user@example.com"),
        },
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    log::init();
    let config = Config::read()?;
    let client = Client::new(config.client_id, config.client_secret);
    let secrets = Secrets::new(config.certificate_id, config.private_key);
    let mut tl = TlBuilder::new(secrets, client)
        .with_auth_server(config.auth_server_uri)
        .with_environment_uri(config.environment_uri)
        .build();
    let mut handle = tl.create_payment(&payment()).await?;

    dbg!(handle.authorization_url());

    loop {
        tokio::time::sleep(Duration::from_secs(3)).await;
        let response = tl.get_payment(&handle.response.id).await?;
        println!("payment status: {:?}", response.status);
    }
    // handle.wait_for_settled();

    // println!("Payment was successful");
    //Ok(())
}
