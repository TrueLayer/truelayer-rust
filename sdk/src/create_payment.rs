use reqwest::Url;
use secrecy::{ExposeSecret, Secret};
use serde::Serialize;
use uuid::Uuid;

use crate::Tl;

pub struct PaymentHandler;

impl PaymentHandler {
    fn pay(&self) {
        todo!()
    }
    /// Retry with the same Idempotency-Key
    fn retry(&self) {
        todo!()
    }
    fn wait_for_authorized(&self) {
        todo!()
    }
    fn wait_for_succeeded(&self) {
        todo!()
    }
    fn wait_for_settled(&self) {
        todo!()
    }
}

pub struct Secrets {
    certificate_id: Secret<String>,
    private_key_pem: Secret<String>,
}

impl Secrets {
    pub fn new(certificate_id: Uuid, private_key_pem: String) -> Self {
        Self {
            private_key_pem: Secret::new(private_key_pem),
            certificate_id: Secret::new(certificate_id.to_string()),
        }
    }

    fn certificate_id(&self) -> &str {
        self.certificate_id.expose_secret()
    }
    fn private_key_pem(&self) -> &[u8] {
        self.private_key_pem.expose_secret().as_bytes()
    }
}

#[derive(Serialize)]
pub enum Currency {
    Gbp,
}

impl ToString for Currency {
    fn to_string(&self) -> String {
        match self {
            Currency::Gbp => "GBP",
        }
        .to_owned()
    }
}

#[derive(Serialize)]
#[serde(tag = "type")]
pub enum PaymentMethod {
    BankTransfer,
}

#[derive(Serialize)]
#[serde(tag = "type")]
pub enum Beneficiary {
    MerchantAccount { id: Uuid, name: String },
}

#[derive(Serialize)]
#[serde(tag = "type")]
pub enum User {
    New {
        name: String,
    },
    Existing {
        id: String, // Maybe this is a Uuid
    },
}

#[derive(Serialize)]
pub struct Payment {
    pub amount_in_minor: u64,
    pub currency: Currency,
    pub payment_method: PaymentMethod,
    pub beneficiary: Beneficiary,
    pub user: User,
}

#[derive(thiserror::Error, Debug)]
pub enum PaymentError {
    /// Error while signing
    #[error("signing error")]
    SigningError(#[from] truelayer_signing::Error),
    /// Error while doing the http request
    #[error("http error")]
    HttpError(#[from] reqwest::Error),
}

static PAYMENTS_PATH: &str = "/payments";

impl Tl {
    pub async fn create_payment(
        &mut self,
        payment: &Payment,
    ) -> Result<PaymentHandler, PaymentError> {
        let payment_bytes = serde_json::to_string(payment)
            .expect("Failed to serialize payment request: This is a bug");
        let payment_bytes = payment_bytes.as_bytes();
        let idempotency_key = Uuid::new_v4().to_string();
        let tl_signature = signature(&self.secrets, payment_bytes, idempotency_key.as_bytes())?;
        let access_token = &self.access_token().await?.access_token.clone();
        let response = self
            .http_client
            .post(self.payments_endpoint())
            .bearer_auth(access_token)
            .header("Tl-Signature", tl_signature)
            .header("Idempotency-Key", idempotency_key)
            .json(&payment)
            .send()
            .await?;

        dbg!(&response.text().await);
        Ok(PaymentHandler)
    }

    fn payments_endpoint(&self) -> Url {
        self.environment_uri
            .join(PAYMENTS_PATH)
            .expect("cannot create payments_path")
    }
}

fn signature(
    secrets: &Secrets,
    payment_body: &[u8],
    idempotency_key: &[u8],
) -> Result<String, truelayer_signing::Error> {
    truelayer_signing::sign_with_pem(secrets.certificate_id(), secrets.private_key_pem())
        .method("POST")
        .path(PAYMENTS_PATH)
        .header("Idempotency-Key", idempotency_key)
        .body(payment_body)
        .sign()
}
