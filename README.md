# truelayer-rust

[![License](https://img.shields.io/:license-mit-blue.svg)](https://truelayer.mit-license.org/)
[![Build](https://github.com/TrueLayer/truelayer-rust/actions/workflows/build.yml/badge.svg)](https://github.com/TrueLayer/truelayer-rust/actions/workflows/build.yml)

[![Crates.io](https://img.shields.io/crates/v/truelayer-rust)](https://crates.io/crates/truelayer-rust)
[![Docs.rs](https://img.shields.io/docsrs/truelayer-rust?label=docs.rs)](https://docs.rs/truelayer-rust/latest/truelayer-rust)

The official [TrueLayer](https://truelayer.com) Rust client provides convenient access to TrueLayer APIs from applications built with Rust.

## Installation

Add the latest version of the library to your project's `Cargo.toml`.

```toml
[dependencies]
truelayer-rust = "0.1" # TODO: update version
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


### Configure Settings


### Initialize TrueLayerClient

> TODO

### Create a payment

> TODO
### Build a link to our hosted createPaymentResponse page

> TODO

## Building locally

## Testing

### Unit and integration tests

You can use `cargo` to run the tests locally:

```shell
cargo test --workspace
```

### Acceptance tests

To execute tests against TrueLayer sandbox environment, you should set the below environment variables:
- `TL_CLIENT_ID`
- `TL_CLIENT_SECRET`
- `TL_SIGNING_KEY_ID`
- `TL_SIGNING_PRIVATE_KEY`

and finally run:

> TODO

## Code linting

To enforce coding style guidelines the project uses [`rustfmt`](https://rust-lang.github.io/rustfmt/).

Bear in mind that the above checks are enforced at CI time, thus
the builds will fail if not compliant.

## Contributing

Contributions are always welcome!

Please adhere to this project's [code of conduct](CODE_OF_CONDUCT.md).

## License

[MIT](LICENSE)