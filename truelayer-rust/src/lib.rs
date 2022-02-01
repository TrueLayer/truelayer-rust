//! Official TrueLayer Rust SDK.

#![deny(missing_debug_implementations)]
#![forbid(unsafe_code)]

pub mod apis;
pub(crate) mod authenticator;
pub mod client;
pub mod error;
mod middlewares;
pub mod pollable;

pub use client::TrueLayerClient;
pub use error::Error;
pub use pollable::{Pollable, PollableUntilTerminalState};
