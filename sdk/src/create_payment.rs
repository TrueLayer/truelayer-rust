use secrecy::{ExposeSecret, Secret};
use serde::Serialize;
use uuid::Uuid;

use crate::Tl;

pub struct Handler;

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
    amount_in_minor: u64,
    currency: Currency,
    payment_method: PaymentMethod,
    beneficiary: Beneficiary,
    user: User,
}

#[derive(thiserror::Error, Debug)]
pub enum PaymentError {
    /// Error while signing
    #[error("signing error")]
    SigningError(#[from] truelayer_signing::Error),
}

impl Tl {
    pub async fn create_payment(
        &self,
        payment: &Payment,
    ) -> Result<Handler, PaymentError> {
        let payment = serde_json::to_string(payment)
            .expect("Failed to serialize payment request: This is a bug");
        let payment = payment.as_bytes();
        let tl_signature =
            truelayer_signing::sign_with_pem(self.secrets.certificate_id(), self.secrets.private_key_pem())
                .method("POST")
                .path("/payments")
                .header("Idempotency-Key", Uuid::new_v4().as_bytes())
                .body(payment)
                .sign()?;

        Ok(Handler)
    }
}
