# TrueLayer Rust

[![License](https://img.shields.io/:license-mit-blue.svg)](https://truelayer.mit-license.org/)
[![Build](https://github.com/TrueLayer/truelayer-rust/actions/workflows/build.yml/badge.svg)](https://github.com/TrueLayer/truelayer-rust/actions/workflows/build.yml)

[![Crates.io](https://img.shields.io/crates/v/truelayer-rust)](https://crates.io/crates/truelayer-rust)
[![Docs.rs](https://img.shields.io/docsrs/truelayer-rust?label=docs.rs)](https://docs.rs/truelayer-rust/latest/truelayer-rust)

The official [TrueLayer](https://truelayer.com) Rust client provides convenient access to TrueLayer APIs from applications built with Rust.

## Installation

Add the latest version of the library to your project's `Cargo.toml`.

```toml
[dependencies]
truelayer-rust = "0.1"
```

Alternatively, you can use [`cargo-edit`](https://crates.io/crates/cargo-edit) if you have it already installed:

```shell
cargo add truelayer-rust
```

## Documentation

For a comprehensive list of examples, check out the official TrueLayer [API documentation](https://docs.truelayer.com).

For the full API reference of this crate, go to [Docs.rs](https://docs.rs/truelayer-rust/latest/truelayer-rust).

## Usage

### Prerequisites

First [sign up](https://console.truelayer.com/) for a developer account. Follow the instructions to set up a new application and obtain your Client ID and Secret. Once the application has been created you must add your application redirected URIs in order to test your integration end-to-end.

Next, generate a signing key pair used to sign API requests.

To generate a private key, run:

```sh
docker run --rm -v ${PWD}:/out -w /out -it alpine/openssl ecparam -genkey -name secp521r1 -noout -out ec512-private-key.pem
```

To obtain the public key, run:

```sh
docker run --rm -v ${PWD}:/out -w /out -it alpine/openssl ec -in ec512-private-key.pem -pubout -out ec512-public-key.pem
```

### Initialize TrueLayerClient

Create a new `TrueLayerClient` and provide your client ID and client secret.

```rust
use truelayer_rust::{TrueLayerClient, apis::auth::Credentials};

let tl = TrueLayerClient::builder(Credentials::ClientCredentials {
    client_id: "some-client-id".to_string(),
    client_secret: "some-client-secret".to_string(),
    scope: "payments".to_string(),
})
.with_signing_key(&config.key_id, config.private_key.into_bytes())
.build();
```

By default, a `TrueLayerClient` connects to the Live environment.
To connect to TrueLayer Sandbox, use `.with_environment(Environment::Sandbox)`.

### Create a payment

```rust
let res = tl
    .payments
    .create(&CreatePaymentRequest {
        amount_in_minor: 100,
        currency: Currency::Gbp,
        payment_method: PaymentMethod::BankTransfer {
            provider_selection: ProviderSelection::UserSelected { filter: None },
            beneficiary: Beneficiary::MerchantAccount {
                merchant_account_id: "some-merchant-id".to_string(),
                account_holder_name: None,
            },
        },
        user: User {
            id: Some(Uuid::new_v4().to_string()),
            name: Some("Some One".to_string()),
            email: Some("some.one@email.com".to_string()),
            phone: None,
        },
    })
    .await?;

tracing::info!("Created new payment: {}", res.id);
```

For more info on all the parameters necessary to create a new payment, please refer to the official
[TrueLayer docs](https://docs.truelayer.com/).

### Build a link to our Hosted Payments Page

```rust
let hpp_link = tl.payments
    .get_hosted_payments_page_link(&res.id, &res.resource_token, config.return_uri.as_str())
    .await;

tracing::info!("HPP Link: {}", hpp_link);
```

### More examples

Look into the [`examples`](./examples) for more example usages of this library.

## Building locally

## Testing

### Unit and integration tests

You can use `cargo` to run the tests locally:

```shell
cargo test
```

### Acceptance tests

To execute tests against TrueLayer sandbox environment, you should set the below environment variables:
- `ACCEPTANCE_TESTS_CLIENT_ID`
- `ACCEPTANCE_TESTS_CLIENT_SECRET`
- `ACCEPTANCE_TESTS_SIGNING_KEY_ID`
- `ACCEPTANCE_TESTS_SIGNING_PRIVATE_KEY`
- `ACCEPTANCE_TESTS_MERCHANT_ACCOUNT_ID`

and finally run:

> cargo test --features acceptance-tests

Acceptance tests are run automatically on every push to main.

## Code linting

To enforce coding style guidelines the project uses [`rustfmt`](https://rust-lang.github.io/rustfmt/).

Bear in mind that the above checks are enforced at CI time, thus
the builds will fail if not compliant.

## Contributing

Contributions are always welcome!

Please adhere to this project's [code of conduct](CODE_OF_CONDUCT.md).

## License

[MIT](LICENSE)