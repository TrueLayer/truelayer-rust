# truelayer-rust-sdk

[![Build](https://github.com/TrueLayer/truelayer-rust-sdk/actions/workflows/build.yml/badge.svg)](https://github.com/TrueLayer/truelayer-rust-sdk/actions/workflows/build.yml)

## TODO

- [ ] Check when to retry all http calls
- [ ] Review logs
- [ ] Right now we propagate `reqwest::error` to the caller.
      Instead, we should analyze this error and map it to a `thiserror:Error` type.
- [ ] mark everything related to the API (enum, structs) as non exhaustive,
      since field can be always added.
- [ ] handle of fields of the requests and responses. Right now only mandatory fields (and a few others are handled).
- [ ] add tests by mocking the API.

## Configuration

Configuration is done by placing a `config.json` file inside the working directory.
The config file is the same of the Insomnia Environment. For example:

```json
{
  "CLIENT_ID": "marco-35022f",
  "CLIENT_SECRET": "542734d9-1dec-491c-bd57-424289682c76",
  "RETURN_URI": "http://localhost:3000/callback",
  "AUTH_SERVER_URI": "https://auth.t7r.dev",
  "ENVIRONMENT_URI": "https://test-pay-api.t7r.dev",
  "REQUIRE_JWS": true,
  "CERTIFICATE_ID": "h28d752d-32cd-8s31-834a-k808fh32ta07",
  "PRIVATE_KEY": "-----BEGIN EC PRIVATE KEY-----\nHIHcAgEBBEIACor8eEyj4Zd5/BABF1uGIhwEBA+8mLpMBOiAxgyzeLDOUxPSIiRk\nvQcy/NftmEEvtNsd+romCg3aX9vd+nFKyLGgBoYFK4EWACOhlYkDgKFMBNFc+JlQ\nh29VuHEDTj9kFxcf6Rm6P1lmZXW4SIeM+N296ERCqrAkzHWPqIi76HYQQ9yOKe8o\n9vwGABFjehWGnGu1JgHVUW2vHAxV+kzmrSex5+YmAygh+XM/m6gp5RjSITajx5Yy\nihH+Jk4yQejBV/+wMyX8dbkhoao/PMhQOPVJ1zWUIg==\n-----END EC PRIVATE KEY-----"
}
```