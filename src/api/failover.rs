//! Failover IP endpoints.

use serde::{Deserialize, Serialize};

use crate::client::RobotClient;
use crate::error::Result;
use crate::transport::Transport;

/// A failover IP address and its current routing target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failover {
    /// The failover IP address.
    pub ip: String,
    /// Netmask of the failover IP, if reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netmask: Option<String>,
    /// Server IP the failover address is currently routed to, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_server_ip: Option<String>,
    /// Owning server IP, if reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_ip: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Envelope {
    failover: Failover,
}

/// Lists all failover IPs on the account.
///
/// # Errors
///
/// Propagates errors from the underlying client: [`crate::Error::Api`] for a
/// non-success status, [`crate::Error::Decode`] for malformed JSON, and
/// [`crate::Error::Transport`] for request failures.
pub fn list<T: Transport>(client: &RobotClient<T>) -> Result<Vec<Failover>> {
    let envelopes: Vec<Envelope> = client.get_json("/failover")?;
    Ok(envelopes
        .into_iter()
        .map(|envelope| envelope.failover)
        .collect())
}

/// Fetches a single failover IP.
///
/// # Errors
///
/// Propagates errors from the underlying client: [`crate::Error::Api`] for a
/// non-success status, [`crate::Error::Decode`] for malformed JSON, and
/// [`crate::Error::Transport`] for request failures.
pub fn get<T: Transport>(client: &RobotClient<T>, ip: &str) -> Result<Failover> {
    let envelope: Envelope = client.get_json(&format!("/failover/{ip}"))?;
    Ok(envelope.failover)
}

/// Routes a failover IP to `active_server_ip`.
///
/// # Errors
///
/// Propagates errors from the underlying client: [`crate::Error::Api`] for a
/// non-success status, [`crate::Error::Decode`] for malformed JSON, and
/// [`crate::Error::Transport`] for request failures.
pub fn route<T: Transport>(
    client: &RobotClient<T>,
    ip: &str,
    active_server_ip: &str,
) -> Result<Failover> {
    let envelope: Envelope = client.post_form(
        &format!("/failover/{ip}"),
        &[("active_server_ip", active_server_ip)],
    )?;
    Ok(envelope.failover)
}
