use dotenv::dotenv;
use sdk::auth::Client;
use sdk::create_payment::Secrets;
use sdk::TlBuilder;
use uuid::Uuid;

#[derive(serde::Deserialize, Debug)]
struct Config {
    client_id: String,
    client_secret: Uuid,
    auth_server_uri: String,
    certificate_id: Uuid,
    private_key: String,
}

impl Config {
    fn read() -> anyhow::Result<Self> {
        dotenv()?;
        let config = envy::from_env::<Self>()?;
        Ok(config)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::read()?;
    let client = Client::new(config.client_id, config.client_secret);
    let secrets = Secrets::new(config.certificate_id, config.private_key);
    let tl = TlBuilder::new(secrets, client);
    Ok(())
}
