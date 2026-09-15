//! Traffic statistics endpoint.
//!
//! The Robot API returns a bare object (no outer envelope):
//!
//! ```json
//! {"type":"month","from":"2023-06-01","to":"2023-06-30",
//!  "data":{"192.0.2.1":{"01":{"in":0.0,"out":0.0,"sum":0.0}}}}
//! ```

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::client::RobotClient;
use crate::error::Result;
use crate::transport::Transport;

/// Traffic volume for a single interval, in GiB.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrafficStatistic {
    /// Ingress (incoming) traffic.
    #[serde(rename = "in")]
    pub ingress: f64,
    /// Egress (outgoing) traffic.
    #[serde(rename = "out")]
    pub egress: f64,
    /// Total traffic.
    #[serde(rename = "sum")]
    pub total: f64,
}

/// Traffic statistics keyed by IP and interval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Traffic {
    /// Range type, for example `month`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Start of the range.
    pub from: String,
    /// End of the range.
    pub to: String,
    /// Statistics keyed by IP address, then by interval.
    pub data: BTreeMap<String, BTreeMap<String, TrafficStatistic>>,
}

/// Queries traffic statistics for `ips` over the given range.
///
/// `kind` is one of `day`, `month`, or `year`; `from` and `to` are formatted as
/// documented for that range (for example `2023-06-01`).
///
/// # Errors
///
/// Propagates errors from the underlying client: [`crate::Error::Api`] for a
/// non-success status, [`crate::Error::Decode`] for malformed JSON, and
/// [`crate::Error::Transport`] for request failures.
pub fn query<T: Transport>(
    client: &RobotClient<T>,
    kind: &str,
    from: &str,
    to: &str,
    ips: &[String],
) -> Result<Traffic> {
    let mut form: Vec<(&str, &str)> = vec![("type", kind), ("from", from), ("to", to)];
    for ip in ips {
        form.push(("ip[]", ip.as_str()));
    }
    form.push(("single_values", "true"));
    client.post_form("/traffic", &form)
}
