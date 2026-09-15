//! vSwitch endpoints.
//!
//! Unlike most Robot endpoints, vSwitch responses are **not** wrapped in an
//! outer object: `/vswitch` returns a bare array and `/vswitch/{id}` a bare
//! object.

use serde::{Deserialize, Serialize};

use crate::client::RobotClient;
use crate::error::Result;
use crate::transport::Transport;

/// Connection status of a server on a vSwitch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    /// Connected and ready.
    #[serde(rename = "ready")]
    Ready,
    /// Connecting or disconnecting.
    #[serde(rename = "in process", alias = "processing")]
    InProcess,
    /// Connect or disconnect failed.
    #[serde(rename = "failed")]
    Failed,
    /// A status this client does not know about.
    #[serde(other)]
    Unknown,
}

/// A server connected to a vSwitch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VSwitchServer {
    /// Server number.
    #[serde(rename = "server_number")]
    pub server_number: u32,
    /// Connection status.
    pub status: ConnectionStatus,
    /// Server IP, if reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_ip: Option<String>,
    /// Server IPv6 network, if reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_ipv6_net: Option<String>,
}

/// A subnet attached to a vSwitch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subnet {
    /// Network address.
    pub ip: String,
    /// Netmask length.
    pub mask: u8,
}

/// A Cloud Network connected to a vSwitch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudNetwork {
    /// Cloud Network ID.
    pub id: u32,
    /// Network address.
    pub ip: String,
    /// Netmask length.
    pub mask: u8,
}

/// A vSwitch. The `server`, `subnet` and `cloud_network` fields are only
/// populated by [`get`]; [`list`] returns the summary fields only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VSwitch {
    /// Unique vSwitch ID.
    pub id: u32,
    /// vSwitch name.
    pub name: String,
    /// VLAN ID (4000..=4091).
    pub vlan: u16,
    /// Whether the vSwitch has been cancelled.
    pub cancelled: bool,
    /// Connected servers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub server: Vec<VSwitchServer>,
    /// Attached subnets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subnet: Vec<Subnet>,
    /// Connected Cloud Networks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cloud_network: Vec<CloudNetwork>,
}

/// Lists all vSwitches on the account.
///
/// # Errors
///
/// Propagates errors from the underlying client: [`crate::Error::Api`] for a
/// non-success status, [`crate::Error::Decode`] for malformed JSON, and
/// [`crate::Error::Transport`] for request failures.
pub fn list<T: Transport>(client: &RobotClient<T>) -> Result<Vec<VSwitch>> {
    client.get_json("/vswitch")
}

/// Fetches a single vSwitch, including connected servers and subnets.
///
/// # Errors
///
/// Propagates errors from the underlying client: [`crate::Error::Api`] for a
/// non-success status, [`crate::Error::Decode`] for malformed JSON, and
/// [`crate::Error::Transport`] for request failures.
pub fn get<T: Transport>(client: &RobotClient<T>, id: u32) -> Result<VSwitch> {
    client.get_json(&format!("/vswitch/{id}"))
}

/// Creates a vSwitch with the given name and VLAN ID.
///
/// # Errors
///
/// Propagates errors from the underlying client: [`crate::Error::Api`] for a
/// non-success status, [`crate::Error::Decode`] for malformed JSON, and
/// [`crate::Error::Transport`] for request failures.
pub fn create<T: Transport>(client: &RobotClient<T>, name: &str, vlan: u16) -> Result<VSwitch> {
    let vlan = vlan.to_string();
    client.post_form("/vswitch", &[("name", name), ("vlan", &vlan)])
}

/// Connects servers to a vSwitch.
///
/// # Errors
///
/// Returns [`crate::Error::Api`] for a non-success status or
/// [`crate::Error::Transport`] if the request fails.
pub fn connect<T: Transport>(client: &RobotClient<T>, id: u32, servers: &[u32]) -> Result<()> {
    let values: Vec<String> = servers.iter().map(u32::to_string).collect();
    let form: Vec<(&str, &str)> = values.iter().map(|v| ("server[]", v.as_str())).collect();
    client.post_form_ok(&format!("/vswitch/{id}/server"), &form)
}

/// Disconnects servers from a vSwitch.
///
/// # Errors
///
/// Returns [`crate::Error::Api`] for a non-success status or
/// [`crate::Error::Transport`] if the request fails.
pub fn disconnect<T: Transport>(client: &RobotClient<T>, id: u32, servers: &[u32]) -> Result<()> {
    let values: Vec<String> = servers.iter().map(u32::to_string).collect();
    let form: Vec<(&str, &str)> = values.iter().map(|v| ("server[]", v.as_str())).collect();
    client.delete_form(&format!("/vswitch/{id}/server"), &form)
}

/// Cancels a vSwitch immediately.
///
/// # Errors
///
/// Returns [`crate::Error::Api`] for a non-success status or
/// [`crate::Error::Transport`] if the request fails.
pub fn cancel<T: Transport>(client: &RobotClient<T>, id: u32) -> Result<()> {
    client.delete_form(&format!("/vswitch/{id}"), &[("cancellation_date", "now")])
}
