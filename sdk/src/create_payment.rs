use std::time::Duration;

use reqwest::Url;
use secrecy::{ExposeSecret, Secret};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Tl;

#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentStatus {
    AuthorizationRequired,
    Authorizing,
    Failed,
    Authorized,
    /// The payment has been successfully executed by the provider (i.e. the sending bank).
    Succeeded,
    /// The payment has arrived in your merchant account. (*only for payments where the beneficiary.type is merchant_account).
    Settled,
}

pub struct PaymentHandler {
    pub response: PaymentResponse,
}

#[derive(thiserror::Error, Debug)]
pub enum WaitForStatusError {
    #[error("received a different final state: `{0:?}`")]
    MismatchedState(PaymentStatus),
    #[error("error while doing an http call")]
    HttpError(#[from] reqwest::Error),
}

impl PaymentHandler {
    /// Retry with the same Idempotency-Key
    pub fn retry(&mut self) {
        todo!()
    }
    pub fn wait_for_authorized(&mut self) {
        todo!()
    }
    pub async fn wait_for_succeeded(
        &mut self,
        tl: &mut crate::Tl,
    ) -> Result<(), WaitForStatusError> {
        println!("pay here: {}", self.authorization_url());

        loop {
            tokio::time::sleep(Duration::from_secs(3)).await;
            let response = tl.get_payment(&self.response.id).await?;
            println!("payment status: {:?}", response.status);
            match response.status {
                PaymentStatus::Succeeded | PaymentStatus::Settled => break Ok(()),
                PaymentStatus::Failed => {
                    return Err(WaitForStatusError::MismatchedState(PaymentStatus::Failed))
                }
                _ => (),
            }
        }
    }
    pub fn wait_for_settled(&mut self) {
        todo!()
    }
    pub fn authorization_url(&self) -> String {
        // TODO return_uri should be read from configuration
        format!(
            "https://checkout.t7r.dev/payments#payment_id={}&resource_token={}&return_uri=https://console.t7r.dev/redirect-page",
            self.response.id,
            self.response.resource_token,
        )
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
#[serde(rename_all = "UPPERCASE")]
pub enum Currency {
    Gbp,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PaymentMethod {
    BankTransfer,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Beneficiary {
    MerchantAccount { id: Uuid, name: String },
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum User {
    New {
        name: String,
        #[serde(flatten)]
        info: NewUserInfo,
    },
    Existing {
        id: String, // Maybe this is a Uuid
    },
}

#[derive(Serialize)]
pub struct NewUserInfo {
    email: Option<String>,
    phone: Option<String>,
}

impl NewUserInfo {
    pub fn with_email(email: impl AsRef<str>) -> Self {
        Self {
            email: Some(email.as_ref().to_string()),
            phone: None,
        }
    }

    pub fn with_phone(phone: impl AsRef<str>) -> Self {
        Self {
            phone: Some(phone.as_ref().to_string()),
            email: None,
        }
    }

    pub fn with_email_and_phone(email: impl AsRef<str>, phone: impl AsRef<str>) -> Self {
        Self {
            email: Some(email.as_ref().to_string()),
            phone: Some(phone.as_ref().to_string()),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
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

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PaymentResponse {
    pub amount_in_minor: u32,
    pub id: Uuid,
    pub payment_method: PaymentMethod,
    pub resource_token: String,
}

static PAYMENTS_PATH: &str = "/payments";

impl Tl {
    pub async fn create_payment(
        &mut self,
        payment: &Payment,
    ) -> Result<PaymentHandler, PaymentError> {
        let payment_json = serde_json::to_string(payment)
            .expect("Failed to serialize payment request: This is a bug");
        let payment_bytes = payment_json.as_bytes();
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
            .await?
            .json::<PaymentResponse>()
            .await?;

        Ok(PaymentHandler { response })
    }

    pub fn payments_endpoint(&self) -> Url {
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

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn payment_body_is_serialized_with_expected_json() {
        let merchant_id = "c54104a5-fdd1-4277-8793-dbfa511c898b";
        let merchant_name = "First Last";
        let user_email = "user@example.com";
        let expected_json = serde_json::json!({
            "amount_in_minor": 1,
            "currency": "GBP",
            "payment_method": {
                "type": "bank_transfer"
            },
            "beneficiary": {
                "type": "merchant_account",
                "id": merchant_id,
                "name": merchant_name
            },
            "user": {
                "type": "new",
                "name": "username",
                "email": user_email,
                "phone": null
            }
        });

        let actual_payment = Payment {
            amount_in_minor: 1,
            currency: Currency::Gbp,
            payment_method: PaymentMethod::BankTransfer,
            beneficiary: Beneficiary::MerchantAccount {
                id: Uuid::from_str(merchant_id).unwrap(),
                name: merchant_name.to_owned(),
            },
            user: User::New {
                name: "username".to_string(),
                info: NewUserInfo::with_email(user_email),
            },
        };

        assert_eq!(
            expected_json,
            serde_json::from_str::<serde_json::Value>(
                &serde_json::to_string(&actual_payment).unwrap()
            )
            .unwrap()
        )
    }
}
