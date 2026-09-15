//! Reverse DNS endpoints.

use serde::{Deserialize, Serialize};

use crate::client::RobotClient;
use crate::error::Result;
use crate::transport::Transport;

/// A reverse DNS entry for an IP address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rdns {
    /// The IP address the entry belongs to.
    pub ip: String,
    /// The PTR record value, if set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ptr: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    rdns: T,
}

/// Fetches the reverse DNS entry for `ip`.
///
/// # Errors
///
/// Propagates errors from the underlying client: [`crate::Error::Api`] for a
/// non-success status, [`crate::Error::Decode`] for malformed JSON, and
/// [`crate::Error::Transport`] for request failures.
pub fn get<T: Transport>(client: &RobotClient<T>, ip: &str) -> Result<Rdns> {
    let envelope: Envelope<Rdns> = client.get_json(&format!("/rdns/{ip}"))?;
    Ok(envelope.rdns)
}

/// Sets the PTR record for `ip`.
///
/// # Errors
///
/// Propagates errors from the underlying client: [`crate::Error::Api`] for a
/// non-success status, [`crate::Error::Decode`] for malformed JSON, and
/// [`crate::Error::Transport`] for request failures.
pub fn set<T: Transport>(client: &RobotClient<T>, ip: &str, ptr: &str) -> Result<Rdns> {
    let envelope: Envelope<Rdns> = client.post_form(&format!("/rdns/{ip}"), &[("ptr", ptr)])?;
    Ok(envelope.rdns)
}
