//! `metalctl-mcp` — a Model Context Protocol server for the Hetzner Robot API.
//!
//! It exposes every `metalctl` capability as an MCP tool over stdio. Read-only
//! tools run directly; destructive tools (resets, cancels, route changes) refuse
//! to run unless called with `confirm = true`.

use std::sync::Arc;

use anyhow::Result;
use metalctl::api;
use metalctl::{
    Credentials, HttpRequest, HttpResponse, RobotClient, Transport, UreqTransport, DEFAULT_BASE_URL,
};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
};
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Adapts a shared, boxed transport to the client's `Transport` bound.
///
/// This is what makes the MCP server testable offline: tests build a
/// `Metalctl` around a mock transport instead of the network.
#[derive(Clone)]
struct SharedTransport(Arc<dyn Transport + Send + Sync>);

impl Transport for SharedTransport {
    fn execute(
        &self,
        request: &HttpRequest,
        authorization: &str,
    ) -> metalctl::Result<HttpResponse> {
        self.0.execute(request, authorization)
    }
}

/// The concrete client type used by the server.
type Client = RobotClient<SharedTransport>;

/// The MCP server state: a shared, synchronous Robot client and the tool router.
#[derive(Clone)]
struct Metalctl {
    client: Arc<Client>,
    /// Read by the code generated from `#[tool_handler]`.
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

impl Metalctl {
    /// Builds a server around an existing client (used by tests and `main`).
    fn with_client(client: Arc<Client>) -> Self {
        Self {
            client,
            tool_router: Self::tool_router(),
        }
    }

    /// Builds a server from credentials, using the real `ureq` transport.
    fn from_credentials(credentials: Credentials) -> Self {
        let transport = SharedTransport(Arc::new(UreqTransport::new()));
        let client = Arc::new(RobotClient::with_transport(
            DEFAULT_BASE_URL,
            credentials,
            transport,
        ));
        Self::with_client(client)
    }
}

/// Runs a blocking `metalctl` call on the blocking thread pool.
///
/// The inner `Result` is the Robot call's outcome; only a worker-join failure is
/// surfaced as a JSON-RPC error.
async fn call<T, F>(client: Arc<Client>, f: F) -> Result<metalctl::Result<T>, McpError>
where
    T: Send + 'static,
    F: FnOnce(&Client) -> metalctl::Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(move || f(&client))
        .await
        .map_err(|error| McpError::internal_error(format!("worker join error: {error}"), None))
}

/// Maps a Robot call outcome to a tool result.
///
/// Execution failures (transport, auth, 404, rate limit, ...) become
/// `CallToolResult { is_error: true }` — the MCP representation for a tool that
/// ran and failed. They are *not* JSON-RPC protocol errors, which MCP reserves
/// for malformed requests. The confirmation refusal takes the same shape.
fn report<T: Serialize>(result: metalctl::Result<T>) -> Result<CallToolResult, McpError> {
    match result {
        Ok(value) => json_result(&value),
        Err(error) => Ok(tool_error(&error)),
    }
}

/// Like [`report`] for calls that produce no response body.
fn report_unit(result: metalctl::Result<()>, message: &str) -> CallToolResult {
    match result {
        Ok(()) => ok_result(message),
        Err(error) => tool_error(&error),
    }
}

fn tool_error(error: &metalctl::Error) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(error.to_string())])
}

fn json_result<T: Serialize>(value: &T) -> Result<CallToolResult, McpError> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| McpError::internal_error(error.to_string(), None))?;
    Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
}

fn ok_result(message: &str) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(message.to_string())])
}

/// Refuses a destructive operation unless `confirm` is set.
///
/// Returns the tool result to hand back on refusal. A missing confirmation is a
/// tool-level error (`is_error`), not a JSON-RPC protocol error, so clients
/// surface it consistently with other execution failures.
fn require_confirm(confirm: bool, action: &str) -> Result<(), CallToolResult> {
    if confirm {
        Ok(())
    } else {
        Err(CallToolResult::error(vec![ContentBlock::text(format!(
            "refusing to {action}: call again with confirm=true once the user has approved"
        ))]))
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
    // ----- read-only -------------------------------------------------------

    #[tool(
        description = "List all dedicated servers on the Hetzner Robot account",
        annotations(read_only_hint = true)
    )]
    async fn server_list(&self) -> Result<CallToolResult, McpError> {
        let result = call(self.client.clone(), api::server::list).await?;
        report(result)
    }

    #[tool(
        description = "Show a single dedicated server by its server number",
        annotations(read_only_hint = true)
    )]
    async fn server_get(
        &self,
        Parameters(p): Parameters<ServerNumber>,
    ) -> Result<CallToolResult, McpError> {
        let result = call(self.client.clone(), move |c| {
            api::server::get(c, p.server_number)
        })
        .await?;
        report(result)
    }

    #[tool(
        description = "Show the reverse DNS PTR record for an IP address",
        annotations(read_only_hint = true)
    )]
    async fn rdns_get(
        &self,
        Parameters(p): Parameters<IpParam>,
    ) -> Result<CallToolResult, McpError> {
        let result = call(self.client.clone(), move |c| api::rdns::get(c, &p.ip)).await?;
        report(result)
    }

    #[tool(
        description = "List the reset methods available for a dedicated server",
        annotations(read_only_hint = true)
    )]
    async fn reset_methods(
        &self,
        Parameters(p): Parameters<ServerNumber>,
    ) -> Result<CallToolResult, McpError> {
        let result = call(self.client.clone(), move |c| {
            api::reset::options(c, p.server_number)
        })
        .await?;
        report(result)
    }

    #[tool(
        description = "Show the current rescue system configuration for a server",
        annotations(read_only_hint = true)
    )]
    async fn boot_rescue_get(
        &self,
        Parameters(p): Parameters<ServerNumber>,
    ) -> Result<CallToolResult, McpError> {
        let result = call(self.client.clone(), move |c| {
            api::boot::rescue(c, p.server_number)
        })
        .await?;
        report(result)
    }

    #[tool(
        description = "List all failover IPs on the Hetzner Robot account",
        annotations(read_only_hint = true)
    )]
    async fn failover_list(&self) -> Result<CallToolResult, McpError> {
        let result = call(self.client.clone(), api::failover::list).await?;
        report(result)
    }

    #[tool(
        description = "Show a single failover IP and its current routing target",
        annotations(read_only_hint = true)
    )]
    async fn failover_get(
        &self,
        Parameters(p): Parameters<IpParam>,
    ) -> Result<CallToolResult, McpError> {
        let result = call(self.client.clone(), move |c| api::failover::get(c, &p.ip)).await?;
        report(result)
    }

    #[tool(
        description = "Query traffic statistics for IPs or subnets over a day, month or year range",
        annotations(read_only_hint = true)
    )]
    async fn traffic_query(
        &self,
        Parameters(p): Parameters<TrafficQuery>,
    ) -> Result<CallToolResult, McpError> {
        let result = call(self.client.clone(), move |c| {
            api::traffic::query(c, &p.kind, &p.from, &p.to, &p.ips)
        })
        .await?;
        report(result)
    }

    #[tool(
        description = "List all vSwitches on the Hetzner Robot account",
        annotations(read_only_hint = true)
    )]
    async fn vswitch_list(&self) -> Result<CallToolResult, McpError> {
        let result = call(self.client.clone(), api::vswitch::list).await?;
        report(result)
    }

    #[tool(
        description = "Show a single vSwitch, including connected servers",
        annotations(read_only_hint = true)
    )]
    async fn vswitch_get(
        &self,
        Parameters(p): Parameters<IdParam>,
    ) -> Result<CallToolResult, McpError> {
        let result = call(self.client.clone(), move |c| api::vswitch::get(c, p.id)).await?;
        report(result)
    }

    // ----- destructive (require confirm=true) ------------------------------

    #[tool(
        description = "Set the reverse DNS PTR record for an IP. Destructive: requires confirm=true.",
        annotations(destructive_hint = true)
    )]
    async fn rdns_set(
        &self,
        Parameters(p): Parameters<RdnsSet>,
    ) -> Result<CallToolResult, McpError> {
        if let Err(refusal) = require_confirm(p.confirm, "change reverse DNS") {
            return Ok(refusal);
        }
        let result = call(self.client.clone(), move |c| {
            api::rdns::set(c, &p.ip, &p.ptr)
        })
        .await?;
        report(result)
    }

    #[tool(
        description = "Reset (sw/hw/power) a dedicated server. Destructive: requires confirm=true.",
        annotations(destructive_hint = true)
    )]
    async fn reset_run(
        &self,
        Parameters(p): Parameters<ResetRun>,
    ) -> Result<CallToolResult, McpError> {
        if let Err(refusal) = require_confirm(p.confirm, "reset the server") {
            return Ok(refusal);
        }
        let kind: api::reset::ResetType = p.kind.into();
        let result = call(self.client.clone(), move |c| {
            api::reset::execute(c, p.server_number, kind)
        })
        .await?;
        report(result)
    }

    #[tool(
        description = "Activate the rescue system for a server. Destructive: requires confirm=true.",
        annotations(destructive_hint = true)
    )]
    async fn boot_rescue_activate(
        &self,
        Parameters(p): Parameters<RescueActivate>,
    ) -> Result<CallToolResult, McpError> {
        if let Err(refusal) = require_confirm(p.confirm, "activate the rescue system") {
            return Ok(refusal);
        }
        let result = call(self.client.clone(), move |c| {
            api::boot::activate_rescue(c, p.server_number, &p.os, &p.arch, &p.authorized_keys)
        })
        .await?;
        report(result)
    }

    #[tool(
        description = "Deactivate the rescue system for a server. Destructive: requires confirm=true.",
        annotations(destructive_hint = true)
    )]
    async fn boot_rescue_deactivate(
        &self,
        Parameters(p): Parameters<ServerConfirm>,
    ) -> Result<CallToolResult, McpError> {
        if let Err(refusal) = require_confirm(p.confirm, "deactivate the rescue system") {
            return Ok(refusal);
        }
        let result = call(self.client.clone(), move |c| {
            api::boot::deactivate_rescue(c, p.server_number)
        })
        .await?;
        Ok(report_unit(result, "rescue system deactivated"))
    }

    #[tool(
        description = "Route a failover IP to a target server IP. Destructive: requires confirm=true.",
        annotations(destructive_hint = true)
    )]
    async fn failover_route(
        &self,
        Parameters(p): Parameters<FailoverRoute>,
    ) -> Result<CallToolResult, McpError> {
        if let Err(refusal) = require_confirm(p.confirm, "move the failover IP") {
            return Ok(refusal);
        }
        let result = call(self.client.clone(), move |c| {
            api::failover::route(c, &p.ip, &p.target)
        })
        .await?;
        report(result)
    }

    #[tool(
        description = "Create a vSwitch. Destructive: requires confirm=true.",
        annotations(destructive_hint = true)
    )]
    async fn vswitch_create(
        &self,
        Parameters(p): Parameters<VSwitchCreate>,
    ) -> Result<CallToolResult, McpError> {
        if let Err(refusal) = require_confirm(p.confirm, "create a vSwitch") {
            return Ok(refusal);
        }
        let result = call(self.client.clone(), move |c| {
            api::vswitch::create(c, &p.name, p.vlan)
        })
        .await?;
        report(result)
    }

    #[tool(
        description = "Connect servers to a vSwitch. Destructive: requires confirm=true.",
        annotations(destructive_hint = true)
    )]
    async fn vswitch_connect(
        &self,
        Parameters(p): Parameters<VSwitchServers>,
    ) -> Result<CallToolResult, McpError> {
        if let Err(refusal) = require_confirm(p.confirm, "connect servers to a vSwitch") {
            return Ok(refusal);
        }
        let result = call(self.client.clone(), move |c| {
            api::vswitch::connect(c, p.id, &p.servers)
        })
        .await?;
        Ok(report_unit(result, "servers connected"))
    }

    #[tool(
        description = "Disconnect servers from a vSwitch. Destructive: requires confirm=true.",
        annotations(destructive_hint = true)
    )]
    async fn vswitch_disconnect(
        &self,
        Parameters(p): Parameters<VSwitchServers>,
    ) -> Result<CallToolResult, McpError> {
        if let Err(refusal) = require_confirm(p.confirm, "disconnect servers from a vSwitch") {
            return Ok(refusal);
        }
        let result = call(self.client.clone(), move |c| {
            api::vswitch::disconnect(c, p.id, &p.servers)
        })
        .await?;
        Ok(report_unit(result, "servers disconnected"))
    }

    #[tool(
        description = "Cancel a vSwitch immediately. Destructive: requires confirm=true.",
        annotations(destructive_hint = true)
    )]
    async fn vswitch_cancel(
        &self,
        Parameters(p): Parameters<VSwitchCancel>,
    ) -> Result<CallToolResult, McpError> {
        if let Err(refusal) = require_confirm(p.confirm, "cancel the vSwitch") {
            return Ok(refusal);
        }
        let result = call(self.client.clone(), move |c| api::vswitch::cancel(c, p.id)).await?;
        Ok(report_unit(result, "vSwitch cancelled"))
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
    let server = Metalctl::from_credentials(Credentials::from_env()?);
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

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

    /// A transport that records every request and answers from fixtures.
    #[derive(Default)]
    struct RecordingTransport {
        requests: Mutex<Vec<HttpRequest>>,
    }

    impl RecordingTransport {
        fn requests(&self) -> Vec<HttpRequest> {
            self.requests.lock().unwrap().clone()
        }
    }

    impl Transport for RecordingTransport {
        fn execute(
            &self,
            request: &HttpRequest,
            _authorization: &str,
        ) -> metalctl::Result<HttpResponse> {
            self.requests.lock().unwrap().push(request.clone());
            let (status, body) = if request.url.ends_with("/server") {
                (
                    200,
                    r#"[{"server":{"server_number":321,"server_name":"alpha","server_ip":"192.0.2.1"}}]"#
                        .to_owned(),
                )
            } else if request.url.contains("/rdns/") {
                (
                    401,
                    r#"{"error":{"status":401,"code":"UNAUTHORIZED","message":"unauthorized"}}"#
                        .to_owned(),
                )
            } else {
                (200, "{}".to_owned())
            };
            Ok(HttpResponse { status, body })
        }
    }

    fn test_server() -> (Metalctl, Arc<RecordingTransport>) {
        let transport = Arc::new(RecordingTransport::default());
        let credentials = Credentials::new("user", "pass").unwrap();
        let client = Arc::new(RobotClient::with_transport(
            "http://stub",
            credentials,
            SharedTransport(transport.clone()),
        ));
        (Metalctl::with_client(client), transport)
    }

    #[test]
    fn registers_all_tools_with_annotations() {
        let (server, _transport) = test_server();
        let tools = server.tool_router.list_all();
        assert_eq!(tools.len(), 19);

        let read_only = [
            "server_list",
            "server_get",
            "rdns_get",
            "reset_methods",
            "boot_rescue_get",
            "failover_list",
            "failover_get",
            "traffic_query",
            "vswitch_list",
            "vswitch_get",
        ];

        for tool in &tools {
            let annotations = tool.annotations.as_ref().expect("tool annotations");
            if read_only.contains(&tool.name.as_ref()) {
                assert_eq!(annotations.read_only_hint, Some(true), "{}", tool.name);
            } else {
                assert_eq!(annotations.destructive_hint, Some(true), "{}", tool.name);
            }
        }
    }

    #[derive(Clone, Default)]
    struct TestClient;

    impl rmcp::ClientHandler for TestClient {}

    #[tokio::test]
    async fn mcp_end_to_end_destructive_contract() {
        let (server, transport) = test_server();
        let (server_io, client_io) = tokio::io::duplex(8192);
        let server_task = tokio::spawn(async move { server.serve(server_io).await });

        let client = TestClient
            .serve(client_io)
            .await
            .expect("client should connect");

        // tools/list: all 19 tools are exposed.
        let tools = client.list_all_tools().await.expect("tools/list");
        assert_eq!(tools.len(), 19);

        // A read-only call reaches the transport and returns the fixture.
        let listed = client
            .call_tool(rmcp::model::CallToolRequestParams::new("server_list"))
            .await
            .expect("tools/call server_list");
        assert_ne!(listed.is_error, Some(true));
        assert!(serde_json::to_string(&listed).unwrap().contains("alpha"));
        assert_eq!(transport.requests().len(), 1);

        // Destructive tool WITHOUT confirm=true: rejected as a tool error, and
        // the transport must not be reached.
        let rejected = client
            .call_tool(
                rmcp::model::CallToolRequestParams::new("vswitch_cancel")
                    .with_arguments(rmcp::object!({ "id": 50301 })),
            )
            .await
            .expect("a confirmation refusal is a tool result");
        assert_eq!(rejected.is_error, Some(true), "must be an error result");
        assert!(
            serde_json::to_string(&rejected)
                .unwrap()
                .contains("confirm"),
            "the error should explain the missing confirmation"
        );
        assert_eq!(
            transport.requests().len(),
            1,
            "no request may be sent without confirm"
        );

        // Destructive tool WITH confirm=true: accepted and reaches the transport
        // with the expected request.
        let accepted = client
            .call_tool(
                rmcp::model::CallToolRequestParams::new("vswitch_cancel")
                    .with_arguments(rmcp::object!({ "id": 50301, "confirm": true })),
            )
            .await
            .expect("a confirmed cancel is a tool result");
        assert_ne!(accepted.is_error, Some(true));

        let requests = transport.requests();
        assert_eq!(
            requests.len(),
            2,
            "exactly one request for the confirmed cancel"
        );
        let cancel = &requests[1];
        assert_eq!(cancel.method, "DELETE");
        assert_eq!(cancel.url, "http://stub/vswitch/50301");
        assert_eq!(cancel.body.as_deref(), Some("cancellation_date=now"));

        server_task.abort();
    }

    #[tokio::test]
    async fn api_errors_are_tool_error_results() {
        let (server, _transport) = test_server();
        let (server_io, client_io) = tokio::io::duplex(8192);
        let server_task = tokio::spawn(async move { server.serve(server_io).await });

        let client = TestClient
            .serve(client_io)
            .await
            .expect("client should connect");

        let result = client
            .call_tool(
                rmcp::model::CallToolRequestParams::new("rdns_get")
                    .with_arguments(rmcp::object!({ "ip": "192.0.2.1" })),
            )
            .await
            .expect("an API failure is a tool result, not a protocol error");
        assert_eq!(result.is_error, Some(true));
        assert!(serde_json::to_string(&result).unwrap().contains("401"));

        server_task.abort();
    }
}
