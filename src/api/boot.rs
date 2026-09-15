//! Boot configuration endpoints (rescue system).

use serde::{Deserialize, Serialize};

use crate::client::RobotClient;
use crate::error::Result;
use crate::transport::Transport;

/// Rescue system configuration for a server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rescue {
    /// Server number.
    pub server_number: u32,
    /// Operating system of the rescue system, if reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<String>,
    /// Architecture (32 or 64), if reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<u32>,
    /// Whether the rescue system is currently active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
    /// Generated rescue password, present while active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Envelope {
    rescue: Rescue,
}

/// Fetches the current rescue system configuration for a server.
///
/// # Errors
///
/// Propagates errors from the underlying client: [`crate::Error::Api`] for a
/// non-success status, [`crate::Error::Decode`] for malformed JSON, and
/// [`crate::Error::Transport`] for request failures.
pub fn rescue<T: Transport>(client: &RobotClient<T>, number: u32) -> Result<Rescue> {
    let envelope: Envelope = client.get_json(&format!("/boot/{number}/rescue"))?;
    Ok(envelope.rescue)
}

/// Activates the rescue system for a server.
///
/// `authorized_keys` are added as repeated `authorized_key` form fields, which
/// lets the server boot into rescue with SSH access and no password.
///
/// # Errors
///
/// Propagates errors from the underlying client: [`crate::Error::Api`] for a
/// non-success status, [`crate::Error::Decode`] for malformed JSON, and
/// [`crate::Error::Transport`] for request failures.
pub fn activate_rescue<T: Transport>(
    client: &RobotClient<T>,
    number: u32,
    os: &str,
    arch: &str,
    authorized_keys: &[String],
) -> Result<Rescue> {
    let mut form: Vec<(&str, &str)> = vec![("os", os), ("arch", arch)];
    for key in authorized_keys {
        form.push(("authorized_key", key.as_str()));
    }
    let envelope: Envelope = client.post_form(&format!("/boot/{number}/rescue"), &form)?;
    Ok(envelope.rescue)
}

/// Deactivates the rescue system for a server.
///
/// # Errors
///
/// Propagates errors from the underlying client: [`crate::Error::Api`] for a
/// non-success status and [`crate::Error::Transport`] for request failures.
pub fn deactivate_rescue<T: Transport>(client: &RobotClient<T>, number: u32) -> Result<()> {
    client.delete(&format!("/boot/{number}/rescue"))
}
