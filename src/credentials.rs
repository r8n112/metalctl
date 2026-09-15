//! Robot API credentials.

use std::env;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;

use crate::error::{Error, Result};

/// Environment variable holding the Robot webservice username.
pub const USER_ENV: &str = "HETZNER_ROBOT_USER";
/// Environment variable holding the Robot webservice password.
pub const PASSWORD_ENV: &str = "HETZNER_ROBOT_PASSWORD";

/// HTTP Basic credentials for the Hetzner Robot API.
///
/// The password is never printed: [`Debug`](std::fmt::Debug) redacts it.
#[derive(Clone)]
pub struct Credentials {
    username: String,
    password: String,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

impl Credentials {
    /// Creates credentials from a username and password.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidCredentials`] if either value is empty.
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Result<Self> {
        let username = username.into();
        let password = password.into();
        if username.is_empty() || password.is_empty() {
            return Err(Error::InvalidCredentials);
        }
        Ok(Self { username, password })
    }

    /// Reads credentials from [`USER_ENV`] and [`PASSWORD_ENV`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::MissingCredentials`] if either variable is unset or
    /// empty, or [`Error::InvalidCredentials`] if a value is invalid.
    pub fn from_env() -> Result<Self> {
        let username = env::var(USER_ENV).unwrap_or_default();
        let password = env::var(PASSWORD_ENV).unwrap_or_default();
        if username.is_empty() || password.is_empty() {
            return Err(Error::MissingCredentials);
        }
        Self::new(username, password)
    }

    /// Returns the value for the HTTP `Authorization` header.
    #[must_use]
    pub fn authorization(&self) -> String {
        let raw = format!("{}:{}", self.username, self.password);
        format!("Basic {}", STANDARD.encode(raw))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_username() {
        assert!(matches!(
            Credentials::new("", "secret"),
            Err(Error::InvalidCredentials)
        ));
    }

    #[test]
    fn rejects_empty_password() {
        assert!(matches!(
            Credentials::new("user", ""),
            Err(Error::InvalidCredentials)
        ));
    }

    #[test]
    fn builds_basic_authorization_header() {
        // base64("user:pass") == "dXNlcjpwYXNz"
        let credentials = Credentials::new("user", "pass").unwrap();
        assert_eq!(credentials.authorization(), "Basic dXNlcjpwYXNz");
    }

    #[test]
    fn debug_redacts_password() {
        let credentials = Credentials::new("user", "s3cr3t").unwrap();
        let rendered = format!("{credentials:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("s3cr3t"));
    }
}
