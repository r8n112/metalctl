//! `metalctl` — a low-level client and CLI for the Hetzner Robot (bare-metal) API.
//!
//! The crate is deliberately split into a small, testable library and a thin
//! binary:
//!
//! - [`RobotClient`] performs authenticated requests over a pluggable
//!   [`Transport`], so tests run entirely in memory.
//! - [`api`] contains typed endpoints.
//! - the `metalctl` binary is a thin wrapper around the library.
//!
//! # Examples
//!
//! ```
//! use metalctl::Credentials;
//!
//! let credentials = Credentials::new("user", "secret").unwrap();
//! assert_eq!(credentials.authorization(), "Basic dXNlcjpzZWNyZXQ=");
//! ```

#![deny(missing_docs)]

pub mod api;
mod client;
mod credentials;
mod error;
mod transport;

pub use client::{RetryPolicy, RobotClient, DEFAULT_BASE_URL};
pub use credentials::{
    default_config_path, load_config, ConfigCredentials, CredentialSources, Credentials,
    PASSWORD_ENV, USER_ENV,
};
pub use error::{Error, Result};
pub use transport::{HttpRequest, HttpResponse, Transport, UreqTransport, DEFAULT_TIMEOUT};
