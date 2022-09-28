//! The official [TrueLayer](https://truelayer.com) Rust client provides convenient access
//! to TrueLayer APIs from applications built with Rust.
//!
//! Check out also the official TrueLayer [API documentation](https://docs.truelayer.com).
//!
//! # Usage
//!
//! ## Prerequisites
//!
//! First [sign up](https://console.truelayer.com/) for a developer account.
//! Follow the instructions to set up a new application and obtain your Client ID and Secret.
//! Once the application has been created you must add your application redirected URIs in order to test your integration end-to-end.
//!
//! Next, generate a signing key pair used to sign API requests.
//!
//! To generate a private key, run:
//!
//! ```sh
//! docker run --rm -v ${PWD}:/out -w /out -it alpine/openssl ecparam -genkey -name secp521r1 -noout -out ec512-private-key.pem
//! ```
//!
//! To obtain the public key, run:
//!
//! ```sh
//! docker run --rm -v ${PWD}:/out -w /out -it alpine/openssl ec -in ec512-private-key.pem -pubout -out ec512-public-key.pem
//! ```
//!
//! ## Initialize a new `TrueLayerClient`
//!
//! Create a new [`TrueLayerClient`](crate::client::TrueLayerClient) and provide your client ID and client secret.
//!
//! ```rust,no_run
//! # use truelayer_rust::{TrueLayerClient, apis::auth::*};
//! # let private_key = vec![];
//! let tl = TrueLayerClient::builder(Credentials::ClientCredentials {
//!     client_id: "some-client-id".into(),
//!     client_secret: "some-client-secret".into(),
//!     scope: "payments".into(),
//! })
//! .with_signing_key("my-kid", private_key)
//! .build();
//! ```
//!
//! By default, a `TrueLayerClient` connects to the Live environment.
//! To connect to TrueLayer Sandbox, use [`with_environment(Environment::Sandbox)`](crate::client::TrueLayerClientBuilder::with_environment).
//!
//! ## Create a payment
//!
//! ```rust,no_run
//! # use truelayer_rust::{TrueLayerClient, Error, apis::payments::*};
//! # use uuid::Uuid;
//! #
//! # #[tokio::main]
//! # async fn main() -> Result<(), Error> {
//! # let tl: TrueLayerClient = unreachable!();
//! #
//! let create_payment_request = CreatePaymentRequestBuilder::default()
//!     .amount_in_minor(100)
//!     .currency(Currency::Gbp)
//!     .payment_method(PaymentMethod::BankTransfer (
//!         BankTransferBuilder::default()
//!             .provider_selection(ProviderSelection::UserSelected (
//!                 UserSelectedBuilder::default().build().unwrap(),
//!         ))
//!         .beneficiary(Beneficiary::MerchantAccount {
//!             merchant_account_id: "some-merchant-account-id".to_string(),
//!             account_holder_name: None,
//!         })
//!         .build()
//!         .unwrap()
//!     ))
//!     .user(CreatePaymentUserRequest::NewUser(NewUser {
//!         name: Some("Some One".to_string()),
//!         email: Some("some.one@email.com".to_string()),
//!         phone: None,
//!     }))
//!     .build()
//!     .unwrap();
//! let res = tl
//!     .payments
//!     .create(&create_payment_request)
//!     .await?;
//!
//! println!("Created new payment: {}", res.id);
//! # Ok(())
//! # }
//! ```
//!
//! For more info on all the parameters necessary to create a new payment, please refer to the official
//! [TrueLayer docs](https://docs.truelayer.com/).
//!
//! ## Build a link to our Hosted Payments Page
//!
//! ```rust,no_run
//! # use truelayer_rust::{TrueLayerClient, Error, apis::payments::*};
//! #
//! # #[tokio::main]
//! # async fn main() -> Result<(), Error> {
//! # let tl: TrueLayerClient = unreachable!();
//! # let create_payment_request: CreatePaymentRequest = unreachable!();
//! #
//! let res = tl.payments.create(&create_payment_request).await?;
//!
//! let hpp_link = tl.payments
//!     .get_hosted_payments_page_link(&res.id, &res.resource_token, "https://my.return.uri")
//!     .await;
//!
//! println!("HPP Link: {}", hpp_link);
//! # Ok(())
//! # }
//! ```
//!
//! ## Listing Merchant Accounts
//!
//! ```rust,no_run
//! # use truelayer_rust::{TrueLayerClient, Error, apis::payments::*};
//! #
//! # #[tokio::main]
//! # async fn main() -> Result<(), Error> {
//! # let tl: TrueLayerClient = unreachable!();
//! #
//! let merchant_accounts = tl.merchant_accounts.list().await?;
//! for merchant_account in &merchant_accounts {
//!     tracing::info!(
//!         "Merchant Account {}: Balance: {:.2} {}",
//!         merchant_account.id,
//!         merchant_account.available_balance_in_minor as f32 / 100.0,
//!         merchant_account.currency
//!     );
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## More examples
//!
//! Look into the [`examples`](../examples) for more example usages of this library.
//!
//! To run an example, use `cargo run` like this:
//!
//! ```shell
//! cargo run --example create_payment
//! ```

#![deny(missing_debug_implementations)]
#![forbid(unsafe_code)]

pub mod apis;
pub(crate) mod authenticator;
pub mod client;
mod common;
pub mod error;
mod middlewares;
pub mod pollable;

pub use client::TrueLayerClient;
pub use error::Error;
pub use pollable::{Pollable, PollableUntilTerminalState};
