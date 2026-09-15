//! Error types for `metalctl`.

use thiserror::Error;

/// Errors produced by the `metalctl` library.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// Required credentials were not present in the environment.
    #[error("missing credentials: set HETZNER_ROBOT_USER and HETZNER_ROBOT_PASSWORD")]
    MissingCredentials,

    /// Credentials were provided but are empty.
    #[error("invalid credentials: username and password must not be empty")]
    InvalidCredentials,

    /// The HTTP transport failed before a response was received.
    #[error("transport error: {0}")]
    Transport(String),

    /// The Robot API returned a non-success status.
    #[error("robot API error (HTTP {status}): {message}")]
    Api {
        /// HTTP status code returned by the API.
        status: u16,
        /// Human-readable message extracted from the error body.
        message: String,
    },

    /// A response body could not be decoded.
    #[error("failed to decode response: {0}")]
    Decode(String),
}

/// Convenience result alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;
