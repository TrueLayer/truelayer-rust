use dotenv::dotenv;
use sdk::auth::Authentication;
use sdk::auth::Client;
use uuid::Uuid;

#[derive(serde::Deserialize, Debug)]
struct Config {
    client_id: String,
    client_secret: Uuid,
    auth_server_uri: String,
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
    let access_token = Authentication::new(config.auth_server_uri)
        .unwrap()
        .auth(client)
        .await?;
    Ok(())
}
