use sdk::auth::Client;
use uuid::Uuid;

use dotenv::dotenv;

#[derive(serde::Deserialize, Debug)]
struct Config {
    certificate_id: Uuid,
    client_secret: Uuid,
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
    let client = Client::new(config.certificate_id, config.client_secret);
    // sdk::Authentication::new();
    Ok(())
}
