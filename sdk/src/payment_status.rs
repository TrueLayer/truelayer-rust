use reqwest::Url;
use serde::Deserialize;
use uuid::Uuid;

use crate::{create_payment::PaymentStatus, Tl};

#[derive(Deserialize)]
pub struct CreatedPayment {
    pub status: PaymentStatus,
}

impl Tl {
    pub async fn get_payment(
        &mut self,
        payment_id: &Uuid,
    ) -> Result<CreatedPayment, reqwest::Error> {
        let access_token = &self.access_token().await?.access_token.clone();
        let endpoint = self.get_payment_endpoint(payment_id);
        self.http_client
            .get(endpoint)
            .bearer_auth(access_token)
            .send()
            .await?
            .json::<CreatedPayment>()
            .await
    }

    fn get_payment_endpoint(&self, payment_id: &Uuid) -> Url {
        let mut endpoint = self.payments_endpoint().to_string();
        endpoint.push('/');
        Url::parse(&endpoint)
            .unwrap()
            .join(payment_id.to_string().as_str())
            .unwrap()
    }
}
