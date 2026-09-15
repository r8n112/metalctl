//! Server reset endpoints.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::client::RobotClient;
use crate::error::Result;
use crate::transport::Transport;

/// A reset method accepted by the Robot API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetType {
    /// Software reset.
    Software,
    /// Hardware reset.
    Hardware,
    /// Power cycle.
    Power,
}

impl ResetType {
    /// Returns the API value for the reset method.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Software => "sw",
            Self::Hardware => "hw",
            Self::Power => "power",
        }
    }
}

impl fmt::Display for ResetType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Reset methods available for a server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResetOptions {
    /// Server number.
    pub server_number: u32,
    /// Available reset methods, as reported by the API.
    #[serde(rename = "type")]
    pub types: Vec<String>,
}

/// Result of a reset request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResetResult {
    /// Server number.
    pub server_number: u32,
    /// The reset method that was used.
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Deserialize)]
struct OptionsEnvelope {
    reset: ResetOptions,
}

#[derive(Debug, Deserialize)]
struct ResultEnvelope {
    reset: ResetResult,
}

/// Lists the reset methods available for a server.
///
/// # Errors
///
/// Propagates errors from the underlying client: [`crate::Error::Api`] for a
/// non-success status, [`crate::Error::Decode`] for malformed JSON, and
/// [`crate::Error::Transport`] for request failures.
pub fn options<T: Transport>(client: &RobotClient<T>, number: u32) -> Result<ResetOptions> {
    let envelope: OptionsEnvelope = client.get_json(&format!("/reset/{number}"))?;
    Ok(envelope.reset)
}

/// Executes a reset for a server.
///
/// # Errors
///
/// Propagates errors from the underlying client: [`crate::Error::Api`] for a
/// non-success status, [`crate::Error::Decode`] for malformed JSON, and
/// [`crate::Error::Transport`] for request failures.
pub fn execute<T: Transport>(
    client: &RobotClient<T>,
    number: u32,
    kind: ResetType,
) -> Result<ResetResult> {
    let envelope: ResultEnvelope =
        client.post_form(&format!("/reset/{number}"), &[("type", kind.as_str())])?;
    Ok(envelope.reset)
}
