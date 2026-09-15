//! `metalctl-mcp` — a Model Context Protocol server for the Hetzner Robot API.
//!
//! It exposes every `metalctl` capability as an MCP tool over stdio. Read-only
//! tools run directly; destructive tools (resets, cancels, route changes) refuse
//! to run unless called with `confirm = true`.

use std::sync::Arc;

use anyhow::Result;
use metalctl::api;
use metalctl::{Credentials, RobotClient};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
};
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The MCP server state: a shared, synchronous Robot client and the tool router.
#[derive(Clone)]
struct Metalctl {
    client: Arc<RobotClient>,
    /// Read by the code generated from `#[tool_handler]`.
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

/// Runs a blocking `metalctl` call on the blocking thread pool.
async fn blocking<T, F>(client: Arc<RobotClient>, call: F) -> Result<T, McpError>
where
    T: Send + 'static,
    F: FnOnce(&RobotClient) -> metalctl::Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(move || call(&client))
        .await
        .map_err(|error| McpError::internal_error(format!("worker join error: {error}"), None))?
        .map_err(|error| map_error(&error))
}

fn map_error(error: &metalctl::Error) -> McpError {
    match error {
        metalctl::Error::Api { status: 404, .. } => {
            McpError::resource_not_found(error.to_string(), None)
        }
        _ => McpError::internal_error(error.to_string(), None),
    }
}

fn json_result<T: Serialize>(value: &T) -> Result<CallToolResult, McpError> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| McpError::internal_error(error.to_string(), None))?;
    Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
}

fn ok_result(message: &str) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(message.to_string())])
}

fn require_confirm(confirm: bool, action: &str) -> Result<(), McpError> {
    if confirm {
        Ok(())
    } else {
        Err(McpError::invalid_params(
            format!(
                "refusing to {action}: call again with confirm=true once the user has approved"
            ),
            None,
        ))
    }
}

fn default_os() -> String {
    "linux".to_string()
}

fn default_arch() -> String {
    "64".to_string()
}

fn default_kind() -> String {
    "month".to_string()
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ServerNumber {
    /// Robot server number.
    server_number: u32,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct IpParam {
    /// IP address.
    ip: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct IdParam {
    /// vSwitch ID.
    id: u32,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RdnsSet {
    /// IP address.
    ip: String,
    /// New PTR record value.
    ptr: String,
    /// Must be true to apply the change.
    #[serde(default)]
    confirm: bool,
}

/// Reset method.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum ResetKind {
    /// Software reset.
    Sw,
    /// Hardware reset.
    Hw,
    /// Power cycle.
    Power,
}

impl From<ResetKind> for api::reset::ResetType {
    fn from(value: ResetKind) -> Self {
        match value {
            ResetKind::Sw => Self::Software,
            ResetKind::Hw => Self::Hardware,
            ResetKind::Power => Self::Power,
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ResetRun {
    /// Robot server number.
    server_number: u32,
    /// Reset method.
    kind: ResetKind,
    /// Must be true to execute the reset.
    #[serde(default)]
    confirm: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RescueActivate {
    /// Robot server number.
    server_number: u32,
    /// Rescue operating system.
    #[serde(default = "default_os")]
    os: String,
    /// Architecture (32 or 64).
    #[serde(default = "default_arch")]
    arch: String,
    /// Authorised SSH public keys.
    #[serde(default)]
    authorized_keys: Vec<String>,
    /// Must be true to activate the rescue system.
    #[serde(default)]
    confirm: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ServerConfirm {
    /// Robot server number.
    server_number: u32,
    /// Must be true to apply the change.
    #[serde(default)]
    confirm: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct FailoverRoute {
    /// Failover IP address.
    ip: String,
    /// Target server IP the failover address should point at.
    target: String,
    /// Must be true to move the failover IP.
    #[serde(default)]
    confirm: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct VSwitchCreate {
    /// vSwitch name.
    name: String,
    /// VLAN ID (4000..=4091).
    vlan: u16,
    /// Must be true to create the vSwitch.
    #[serde(default)]
    confirm: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct VSwitchServers {
    /// vSwitch ID.
    id: u32,
    /// Server numbers to connect or disconnect.
    servers: Vec<u32>,
    /// Must be true to change membership.
    #[serde(default)]
    confirm: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct VSwitchCancel {
    /// vSwitch ID.
    id: u32,
    /// Must be true to cancel the vSwitch.
    #[serde(default)]
    confirm: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct TrafficQuery {
    /// Range type: `day`, `month` or `year`.
    #[serde(default = "default_kind")]
    kind: String,
    /// Start of the range, for example `2026-09-01`.
    from: String,
    /// End of the range, for example `2026-09-30`.
    to: String,
    /// IP addresses or subnets to query.
    #[serde(default)]
    ips: Vec<String>,
}

#[tool_router]
impl Metalctl {
    fn new(client: Arc<RobotClient>) -> Self {
        Self {
            client,
            tool_router: Self::tool_router(),
        }
    }

    // ----- read-only -------------------------------------------------------

    #[tool(description = "List all dedicated servers on the Hetzner Robot account")]
    async fn server_list(&self) -> Result<CallToolResult, McpError> {
        let servers = blocking(self.client.clone(), api::server::list).await?;
        json_result(&servers)
    }

    #[tool(description = "Show a single dedicated server by its server number")]
    async fn server_get(
        &self,
        Parameters(p): Parameters<ServerNumber>,
    ) -> Result<CallToolResult, McpError> {
        let server = blocking(self.client.clone(), move |c| {
            api::server::get(c, p.server_number)
        })
        .await?;
        json_result(&server)
    }

    #[tool(description = "Show the reverse DNS PTR record for an IP address")]
    async fn rdns_get(
        &self,
        Parameters(p): Parameters<IpParam>,
    ) -> Result<CallToolResult, McpError> {
        let entry = blocking(self.client.clone(), move |c| api::rdns::get(c, &p.ip)).await?;
        json_result(&entry)
    }

    #[tool(description = "List the reset methods available for a dedicated server")]
    async fn reset_methods(
        &self,
        Parameters(p): Parameters<ServerNumber>,
    ) -> Result<CallToolResult, McpError> {
        let options = blocking(self.client.clone(), move |c| {
            api::reset::options(c, p.server_number)
        })
        .await?;
        json_result(&options)
    }

    #[tool(description = "Show the current rescue system configuration for a server")]
    async fn boot_rescue_get(
        &self,
        Parameters(p): Parameters<ServerNumber>,
    ) -> Result<CallToolResult, McpError> {
        let rescue = blocking(self.client.clone(), move |c| {
            api::boot::rescue(c, p.server_number)
        })
        .await?;
        json_result(&rescue)
    }

    #[tool(description = "List all failover IPs on the Hetzner Robot account")]
    async fn failover_list(&self) -> Result<CallToolResult, McpError> {
        let entries = blocking(self.client.clone(), api::failover::list).await?;
        json_result(&entries)
    }

    #[tool(description = "Show a single failover IP and its current routing target")]
    async fn failover_get(
        &self,
        Parameters(p): Parameters<IpParam>,
    ) -> Result<CallToolResult, McpError> {
        let entry = blocking(self.client.clone(), move |c| api::failover::get(c, &p.ip)).await?;
        json_result(&entry)
    }

    #[tool(
        description = "Query traffic statistics for IPs or subnets over a day, month or year range"
    )]
    async fn traffic_query(
        &self,
        Parameters(p): Parameters<TrafficQuery>,
    ) -> Result<CallToolResult, McpError> {
        let traffic = blocking(self.client.clone(), move |c| {
            api::traffic::query(c, &p.kind, &p.from, &p.to, &p.ips)
        })
        .await?;
        json_result(&traffic)
    }

    #[tool(description = "List all vSwitches on the Hetzner Robot account")]
    async fn vswitch_list(&self) -> Result<CallToolResult, McpError> {
        let switches = blocking(self.client.clone(), api::vswitch::list).await?;
        json_result(&switches)
    }

    #[tool(description = "Show a single vSwitch, including connected servers")]
    async fn vswitch_get(
        &self,
        Parameters(p): Parameters<IdParam>,
    ) -> Result<CallToolResult, McpError> {
        let vswitch = blocking(self.client.clone(), move |c| api::vswitch::get(c, p.id)).await?;
        json_result(&vswitch)
    }

    // ----- destructive (require confirm=true) ------------------------------

    #[tool(
        description = "Set the reverse DNS PTR record for an IP. Destructive: requires confirm=true."
    )]
    async fn rdns_set(
        &self,
        Parameters(p): Parameters<RdnsSet>,
    ) -> Result<CallToolResult, McpError> {
        require_confirm(p.confirm, "change reverse DNS")?;
        let entry = blocking(self.client.clone(), move |c| {
            api::rdns::set(c, &p.ip, &p.ptr)
        })
        .await?;
        json_result(&entry)
    }

    #[tool(
        description = "Reset (sw/hw/power) a dedicated server. Destructive: requires confirm=true."
    )]
    async fn reset_run(
        &self,
        Parameters(p): Parameters<ResetRun>,
    ) -> Result<CallToolResult, McpError> {
        require_confirm(p.confirm, "reset the server")?;
        let kind: api::reset::ResetType = p.kind.into();
        let result = blocking(self.client.clone(), move |c| {
            api::reset::execute(c, p.server_number, kind)
        })
        .await?;
        json_result(&result)
    }

    #[tool(
        description = "Activate the rescue system for a server. Destructive: requires confirm=true."
    )]
    async fn boot_rescue_activate(
        &self,
        Parameters(p): Parameters<RescueActivate>,
    ) -> Result<CallToolResult, McpError> {
        require_confirm(p.confirm, "activate the rescue system")?;
        let rescue = blocking(self.client.clone(), move |c| {
            api::boot::activate_rescue(c, p.server_number, &p.os, &p.arch, &p.authorized_keys)
        })
        .await?;
        json_result(&rescue)
    }

    #[tool(
        description = "Deactivate the rescue system for a server. Destructive: requires confirm=true."
    )]
    async fn boot_rescue_deactivate(
        &self,
        Parameters(p): Parameters<ServerConfirm>,
    ) -> Result<CallToolResult, McpError> {
        require_confirm(p.confirm, "deactivate the rescue system")?;
        blocking(self.client.clone(), move |c| {
            api::boot::deactivate_rescue(c, p.server_number)
        })
        .await?;
        Ok(ok_result("rescue system deactivated"))
    }

    #[tool(
        description = "Route a failover IP to a target server IP. Destructive: requires confirm=true."
    )]
    async fn failover_route(
        &self,
        Parameters(p): Parameters<FailoverRoute>,
    ) -> Result<CallToolResult, McpError> {
        require_confirm(p.confirm, "move the failover IP")?;
        let entry = blocking(self.client.clone(), move |c| {
            api::failover::route(c, &p.ip, &p.target)
        })
        .await?;
        json_result(&entry)
    }

    #[tool(description = "Create a vSwitch. Destructive: requires confirm=true.")]
    async fn vswitch_create(
        &self,
        Parameters(p): Parameters<VSwitchCreate>,
    ) -> Result<CallToolResult, McpError> {
        require_confirm(p.confirm, "create a vSwitch")?;
        let vswitch = blocking(self.client.clone(), move |c| {
            api::vswitch::create(c, &p.name, p.vlan)
        })
        .await?;
        json_result(&vswitch)
    }

    #[tool(description = "Connect servers to a vSwitch. Destructive: requires confirm=true.")]
    async fn vswitch_connect(
        &self,
        Parameters(p): Parameters<VSwitchServers>,
    ) -> Result<CallToolResult, McpError> {
        require_confirm(p.confirm, "connect servers to a vSwitch")?;
        blocking(self.client.clone(), move |c| {
            api::vswitch::connect(c, p.id, &p.servers)
        })
        .await?;
        Ok(ok_result("servers connected"))
    }

    #[tool(description = "Disconnect servers from a vSwitch. Destructive: requires confirm=true.")]
    async fn vswitch_disconnect(
        &self,
        Parameters(p): Parameters<VSwitchServers>,
    ) -> Result<CallToolResult, McpError> {
        require_confirm(p.confirm, "disconnect servers from a vSwitch")?;
        blocking(self.client.clone(), move |c| {
            api::vswitch::disconnect(c, p.id, &p.servers)
        })
        .await?;
        Ok(ok_result("servers disconnected"))
    }

    #[tool(description = "Cancel a vSwitch immediately. Destructive: requires confirm=true.")]
    async fn vswitch_cancel(
        &self,
        Parameters(p): Parameters<VSwitchCancel>,
    ) -> Result<CallToolResult, McpError> {
        require_confirm(p.confirm, "cancel the vSwitch")?;
        blocking(self.client.clone(), move |c| api::vswitch::cancel(c, p.id)).await?;
        Ok(ok_result("vSwitch cancelled"))
    }
}

#[tool_handler]
impl ServerHandler for Metalctl {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_protocol_version(ProtocolVersion::V_2024_11_05)
            .with_instructions(
                "Manage Hetzner Robot dedicated servers: servers, reverse DNS, resets, rescue \
                 boot, failover IPs, traffic and vSwitch. Destructive tools require confirm=true."
                    .to_string(),
            )
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let credentials = Credentials::from_env()?;
    let server = Metalctl::new(Arc::new(RobotClient::new(credentials)));
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_kind_maps_to_api_type() {
        assert_eq!(
            api::reset::ResetType::from(ResetKind::Power).as_str(),
            "power"
        );
    }

    #[test]
    fn confirm_is_required() {
        assert!(require_confirm(false, "reset the server").is_err());
        assert!(require_confirm(true, "reset the server").is_ok());
    }

    #[test]
    fn registers_all_tools() {
        let credentials = Credentials::new("user", "pass").unwrap();
        let server = Metalctl::new(Arc::new(RobotClient::new(credentials)));
        let names: Vec<String> = server
            .tool_router
            .list_all()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect();

        for expected in [
            "server_list",
            "server_get",
            "rdns_get",
            "rdns_set",
            "reset_methods",
            "reset_run",
            "boot_rescue_get",
            "boot_rescue_activate",
            "boot_rescue_deactivate",
            "failover_list",
            "failover_get",
            "failover_route",
            "traffic_query",
            "vswitch_list",
            "vswitch_get",
            "vswitch_create",
            "vswitch_connect",
            "vswitch_disconnect",
            "vswitch_cancel",
        ] {
            assert!(
                names.iter().any(|name| name == expected),
                "missing tool {expected}"
            );
        }
        assert_eq!(names.len(), 19);
    }
}
