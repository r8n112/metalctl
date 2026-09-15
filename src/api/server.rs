//! Dedicated server endpoints.

use serde::{Deserialize, Serialize};

use crate::client::RobotClient;
use crate::error::Result;
use crate::transport::Transport;

/// A dedicated server as returned by the Robot API.
///
/// Only the most commonly used fields are modelled; the Robot API returns many
/// more, which are ignored during deserialisation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Server {
    /// Server number, unique within the account.
    pub server_number: u32,
    /// Server name.
    pub server_name: String,
    /// Primary IPv4 address.
    pub server_ip: String,
    /// Assigned IPv6 network, if reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_ipv6_net: Option<String>,
    /// Product name, if reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    /// Current status, if reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Datacenter, if reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dc: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    server: T,
}

/// Lists all servers on the account.
///
/// # Errors
///
/// Propagates errors from the underlying client: [`crate::Error::Api`] for a
/// non-success status, [`crate::Error::Decode`] for malformed JSON, and
/// [`crate::Error::Transport`] for request failures.
pub fn list<T: Transport>(client: &RobotClient<T>) -> Result<Vec<Server>> {
    let envelopes: Vec<Envelope<Server>> = client.get_json("/server")?;
    Ok(envelopes
        .into_iter()
        .map(|envelope| envelope.server)
        .collect())
}

/// Fetches a single server by its server number.
///
/// # Errors
///
/// Propagates errors from the underlying client: [`crate::Error::Api`] for a
/// non-success status, [`crate::Error::Decode`] for malformed JSON, and
/// [`crate::Error::Transport`] for request failures.
pub fn get<T: Transport>(client: &RobotClient<T>, number: u32) -> Result<Server> {
    let envelope: Envelope<Server> = client.get_json(&format!("/server/{number}"))?;
    Ok(envelope.server)
}
