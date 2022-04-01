# Integration and acceptance tests

Integration tests are run against a [local in-memory server](common/mock_server)
mocking the TrueLayer APIs we are interested in. To run integration tests against
this local mock, just run:

```shell
cargo test
```

The same tests can also be run as acceptance tests against the TrueLayer Sandbox
environment enabling the `acceptance-tests` feature when building:

```shell
cargo test --features acceptance-tests
```

In the latter case, the following environment variables must be defined
(these values can be found in the [TrueLayer Console](https://console.truelayer.com)).

- `ACCEPTANCE_TESTS_CLIENT_ID`: Client ID of your application.
- `ACCEPTANCE_TESTS_CLIENT_SECRET`: Client Secret of your application.
- `ACCEPTANCE_TESTS_SIGNING_KEY_ID`: ID of the key registered for request signing.
- `ACCEPTANCE_TESTS_SIGNING_PRIVATE_KEY`: Private Key (PEM formatted) of the public key uploaded on the console.
- `ACCEPTANCE_TESTS_MERCHANT_ACCOUNT_GBP_ID`: ID of your merchant account that will receive GBP funds during the tests.
- `ACCEPTANCE_TESTS_MERCHANT_ACCOUNT_GBP_SWEEPING_IBAN`: Pre-approved IBAN for sweeping tests of your merchant account.