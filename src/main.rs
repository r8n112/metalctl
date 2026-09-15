//! `metalctl` command-line interface.

use std::io::IsTerminal;

use clap::{Parser, Subcommand};
use metalctl::{api, Credentials, Error, Result, RobotClient, UreqTransport};

#[derive(Debug, Parser)]
#[command(
    name = "metalctl",
    version,
    about = "Low-level CLI for the Hetzner Robot API"
)]
struct Cli {
    /// Print raw JSON instead of a human-readable table.
    #[arg(long, global = true)]
    json: bool,

    /// Print what a destructive command would do and exit without sending it.
    #[arg(long, global = true)]
    dry_run: bool,

    /// Skip the interactive confirmation prompt for destructive commands.
    #[arg(long, global = true)]
    yes: bool,

    /// Override the API base URL. Testing hook, not a supported user feature.
    #[arg(long, global = true, hide = true, value_name = "URL")]
    base_url: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Work with dedicated servers.
    Server {
        #[command(subcommand)]
        command: ServerCommand,
    },
    /// Work with reverse DNS entries.
    Rdns {
        #[command(subcommand)]
        command: RdnsCommand,
    },
    /// Reset a dedicated server.
    Reset {
        #[command(subcommand)]
        command: ResetCommand,
    },
    /// Manage boot configuration (rescue system).
    Boot {
        #[command(subcommand)]
        command: BootCommand,
    },
    /// Manage failover IPs.
    Failover {
        #[command(subcommand)]
        command: FailoverCommand,
    },
    /// Query traffic statistics.
    Traffic {
        /// Range type: day, month or year.
        #[arg(long, default_value = "month")]
        kind: String,
        /// Start of the range, for example 2026-09-01.
        #[arg(long)]
        from: String,
        /// End of the range, for example 2026-09-30.
        #[arg(long)]
        to: String,
        /// IP addresses or subnets to query.
        ips: Vec<String>,
    },
    /// Manage vSwitches.
    Vswitch {
        #[command(subcommand)]
        command: VSwitchCommand,
    },
}

#[derive(Debug, Subcommand)]
enum ServerCommand {
    /// List all servers on the account.
    List,
    /// Show a single server by its server number.
    Get {
        /// Server number.
        number: u32,
    },
}

#[derive(Debug, Subcommand)]
enum RdnsCommand {
    /// Show the reverse DNS entry for an IP address.
    Get {
        /// IP address.
        ip: String,
    },
    /// Set the PTR record for an IP address.
    Set {
        /// IP address.
        ip: String,
        /// PTR record value.
        ptr: String,
    },
}

#[derive(Debug, Subcommand)]
enum ResetCommand {
    /// List the reset methods available for a server.
    Methods {
        /// Server number.
        number: u32,
    },
    /// Execute a reset for a server.
    Run {
        /// Server number.
        number: u32,
        /// Reset method.
        #[arg(value_enum)]
        kind: ResetKind,
    },
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum ResetKind {
    /// Software reset.
    Sw,
    /// Hardware reset.
    Hw,
    /// Power cycle.
    Power,
}

impl ResetKind {
    /// Lowercase label used in confirmations and dry-run output.
    fn label(self) -> &'static str {
        match self {
            Self::Sw => "sw",
            Self::Hw => "hw",
            Self::Power => "power",
        }
    }
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

#[derive(Debug, Subcommand)]
enum BootCommand {
    /// Manage the rescue system.
    Rescue {
        #[command(subcommand)]
        command: RescueCommand,
    },
}

#[derive(Debug, Subcommand)]
enum RescueCommand {
    /// Show the current rescue system configuration.
    Get {
        /// Server number.
        number: u32,
    },
    /// Activate the rescue system.
    Activate {
        /// Server number.
        number: u32,
        /// Operating system.
        #[arg(long, default_value = "linux")]
        os: String,
        /// Architecture (32 or 64).
        #[arg(long, default_value = "64")]
        arch: String,
        /// Authorised SSH key (may be repeated).
        #[arg(long = "key")]
        authorized_keys: Vec<String>,
    },
    /// Deactivate the rescue system.
    Deactivate {
        /// Server number.
        number: u32,
    },
}

#[derive(Debug, Subcommand)]
enum VSwitchCommand {
    /// List all vSwitches.
    List,
    /// Show a single vSwitch, including connected servers.
    Get {
        /// vSwitch ID.
        id: u32,
    },
    /// Create a vSwitch.
    Create {
        /// vSwitch name.
        name: String,
        /// VLAN ID (4000..=4091).
        #[arg(long, default_value_t = 4000)]
        vlan: u16,
    },
    /// Connect servers to a vSwitch.
    Connect {
        /// vSwitch ID.
        id: u32,
        /// Server numbers to connect.
        #[arg(required = true)]
        servers: Vec<u32>,
    },
    /// Disconnect servers from a vSwitch.
    Disconnect {
        /// vSwitch ID.
        id: u32,
        /// Server numbers to disconnect.
        #[arg(required = true)]
        servers: Vec<u32>,
    },
    /// Cancel a vSwitch immediately.
    Cancel {
        /// vSwitch ID.
        id: u32,
    },
}

#[derive(Debug, Subcommand)]
enum FailoverCommand {
    /// List all failover IPs on the account.
    List,
    /// Show a single failover IP.
    Get {
        /// Failover IP address.
        ip: String,
    },
    /// Route a failover IP to a server IP.
    Route {
        /// Failover IP address.
        ip: String,
        /// Target server IP address.
        active_server_ip: String,
    },
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// Whether `command` performs a destructive (mutating) operation.
fn is_destructive(command: &Command) -> bool {
    match command {
        Command::Rdns { command } => matches!(command, RdnsCommand::Set { .. }),
        Command::Reset { command } => matches!(command, ResetCommand::Run { .. }),
        Command::Boot { command } => match command {
            BootCommand::Rescue { command } => matches!(
                command,
                RescueCommand::Activate { .. } | RescueCommand::Deactivate { .. }
            ),
        },
        Command::Failover { command } => matches!(command, FailoverCommand::Route { .. }),
        Command::Vswitch { command } => matches!(
            command,
            VSwitchCommand::Create { .. }
                | VSwitchCommand::Connect { .. }
                | VSwitchCommand::Disconnect { .. }
                | VSwitchCommand::Cancel { .. }
        ),
        Command::Server { .. } | Command::Traffic { .. } => false,
    }
}

fn run(cli: &Cli) -> Result<()> {
    let credentials = if cli.dry_run && is_destructive(&cli.command) {
        // A dry-run never sends a request, so it must not require real
        // credentials. The client is never used because `approve` stops first.
        Credentials::new("dry-run", "dry-run")?
    } else {
        Credentials::from_env()?
    };
    let client = match &cli.base_url {
        Some(base_url) => {
            RobotClient::with_transport(base_url.clone(), credentials, UreqTransport::new())
        }
        None => RobotClient::new(credentials),
    };
    dispatch(&client, cli)
}

/// Outcome of the destructive-operation confirmation check.
#[derive(Debug, PartialEq, Eq)]
enum Decision {
    /// `--dry-run`: print the action and do not execute it.
    DryRun,
    /// Confirmed (via `--yes` or an interactive `y`): execute it.
    Proceed,
    /// Not confirmed: refuse without executing.
    Refused,
}

/// Pure confirmation logic (testable without a terminal).
///
/// `--dry-run` always wins. `--yes` skips the prompt. Without either, an
/// affirmative interactive answer is required; a non-interactive session is
/// always refused so an operation can never run because stdin was unavailable.
fn decide(dry_run: bool, yes: bool, interactive: bool, answer: Option<&str>) -> Decision {
    if dry_run {
        return Decision::DryRun;
    }
    if yes {
        return Decision::Proceed;
    }
    if !interactive {
        return Decision::Refused;
    }
    match answer.map(str::trim) {
        Some(answer) if answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes") => {
            Decision::Proceed
        }
        _ => Decision::Refused,
    }
}

/// Confirms a destructive `action`. Returns `Ok(true)` to proceed and
/// `Ok(false)` when `--dry-run` printed the action and it must not be executed.
fn approve(cli: &Cli, action: &str) -> Result<bool> {
    let interactive = std::io::stdin().is_terminal();
    let answer = if interactive && !cli.dry_run && !cli.yes {
        eprint!("{action}? [y/N] ");
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
        Some(line)
    } else {
        None
    };

    match decide(cli.dry_run, cli.yes, interactive, answer.as_deref()) {
        Decision::Proceed => Ok(true),
        Decision::DryRun => {
            eprintln!("dry-run: {action}");
            Ok(false)
        }
        Decision::Refused => Err(Error::ConfirmationRequired {
            action: action.to_string(),
        }),
    }
}

fn dispatch(client: &RobotClient, cli: &Cli) -> Result<()> {
    match &cli.command {
        Command::Server { command } => server_command(client, cli, command),
        Command::Rdns { command } => rdns_command(client, cli, command),
        Command::Reset { command } => reset_command(client, cli, command),
        Command::Boot { command } => boot_command(client, cli, command),
        Command::Failover { command } => failover_command(client, cli, command),
        Command::Traffic {
            kind,
            from,
            to,
            ips,
        } => traffic_command(client, cli, kind, from, to, ips),
        Command::Vswitch { command } => vswitch_command(client, cli, command),
    }
}

fn server_command(client: &RobotClient, cli: &Cli, command: &ServerCommand) -> Result<()> {
    match command {
        ServerCommand::List => {
            let servers = api::server::list(client)?;
            if cli.json {
                print_json(&servers)?;
            } else {
                print_server_table(&servers);
            }
        }
        ServerCommand::Get { number } => {
            let server = api::server::get(client, *number)?;
            if cli.json {
                print_json(&server)?;
            } else {
                print_server(&server);
            }
        }
    }
    Ok(())
}

fn rdns_command(client: &RobotClient, cli: &Cli, command: &RdnsCommand) -> Result<()> {
    match command {
        RdnsCommand::Get { ip } => {
            let entry = api::rdns::get(client, ip)?;
            if cli.json {
                print_json(&entry)?;
            } else {
                print_rdns(&entry);
            }
        }
        RdnsCommand::Set { ip, ptr } => {
            if !approve(cli, &format!("set the PTR record for {ip} to \"{ptr}\""))? {
                return Ok(());
            }
            let entry = api::rdns::set(client, ip, ptr)?;
            if cli.json {
                print_json(&entry)?;
            } else {
                print_rdns(&entry);
            }
        }
    }
    Ok(())
}

fn reset_command(client: &RobotClient, cli: &Cli, command: &ResetCommand) -> Result<()> {
    match command {
        ResetCommand::Methods { number } => {
            let options = api::reset::options(client, *number)?;
            if cli.json {
                print_json(&options)?;
            } else {
                println!("server:  {}", options.server_number);
                println!("methods: {}", options.types.join(", "));
            }
        }
        ResetCommand::Run { number, kind } => {
            if !approve(cli, &format!("reset server {number} ({})", kind.label()))? {
                return Ok(());
            }
            let result = api::reset::execute(client, *number, (*kind).into())?;
            if cli.json {
                print_json(&result)?;
            } else {
                println!("server: {}", result.server_number);
                println!("reset:  {}", result.kind);
            }
        }
    }
    Ok(())
}

fn boot_command(client: &RobotClient, cli: &Cli, command: &BootCommand) -> Result<()> {
    match command {
        BootCommand::Rescue { command } => match command {
            RescueCommand::Get { number } => {
                let rescue = api::boot::rescue(client, *number)?;
                if cli.json {
                    print_json(&rescue)?;
                } else {
                    print_rescue(&rescue);
                }
            }
            RescueCommand::Activate {
                number,
                os,
                arch,
                authorized_keys,
            } => {
                if !approve(
                    cli,
                    &format!("activate the rescue system for server {number}"),
                )? {
                    return Ok(());
                }
                let rescue =
                    api::boot::activate_rescue(client, *number, os, arch, authorized_keys)?;
                if cli.json {
                    print_json(&rescue)?;
                } else {
                    print_rescue(&rescue);
                }
            }
            RescueCommand::Deactivate { number } => {
                if !approve(
                    cli,
                    &format!("deactivate the rescue system for server {number}"),
                )? {
                    return Ok(());
                }
                api::boot::deactivate_rescue(client, *number)?;
                println!("rescue system deactivated for server {number}");
            }
        },
    }
    Ok(())
}

fn failover_command(client: &RobotClient, cli: &Cli, command: &FailoverCommand) -> Result<()> {
    match command {
        FailoverCommand::List => {
            let entries = api::failover::list(client)?;
            if cli.json {
                print_json(&entries)?;
            } else if entries.is_empty() {
                println!("no failover IPs");
            } else {
                for entry in &entries {
                    print_failover_entry(entry);
                }
            }
        }
        FailoverCommand::Get { ip } => {
            let entry = api::failover::get(client, ip)?;
            if cli.json {
                print_json(&entry)?;
            } else {
                print_failover_entry(&entry);
            }
        }
        FailoverCommand::Route {
            ip,
            active_server_ip,
        } => {
            if !approve(
                cli,
                &format!("route failover IP {ip} to {active_server_ip}"),
            )? {
                return Ok(());
            }
            let entry = api::failover::route(client, ip, active_server_ip)?;
            if cli.json {
                print_json(&entry)?;
            } else {
                print_failover_entry(&entry);
            }
        }
    }
    Ok(())
}

fn traffic_command(
    client: &RobotClient,
    cli: &Cli,
    kind: &str,
    from: &str,
    to: &str,
    ips: &[String],
) -> Result<()> {
    let traffic = api::traffic::query(client, kind, from, to, ips)?;
    if cli.json {
        print_json(&traffic)?;
    } else {
        for (ip, buckets) in &traffic.data {
            let total: f64 = buckets.values().map(|statistic| statistic.total).sum();
            println!("{ip}: {total:.2} GiB");
        }
    }
    Ok(())
}

fn vswitch_command(client: &RobotClient, cli: &Cli, command: &VSwitchCommand) -> Result<()> {
    match command {
        VSwitchCommand::List => {
            let switches = api::vswitch::list(client)?;
            if cli.json {
                print_json(&switches)?;
            } else if switches.is_empty() {
                println!("no vSwitches");
            } else {
                for vswitch in &switches {
                    println!("{}  vlan {}  {}", vswitch.id, vswitch.vlan, vswitch.name);
                }
            }
        }
        VSwitchCommand::Get { id } => {
            let vswitch = api::vswitch::get(client, *id)?;
            if cli.json {
                print_json(&vswitch)?;
            } else {
                println!("id:        {}", vswitch.id);
                println!("name:      {}", vswitch.name);
                println!("vlan:      {}", vswitch.vlan);
                println!("cancelled: {}", vswitch.cancelled);
                for server in &vswitch.server {
                    println!("server:    {} ({:?})", server.server_number, server.status);
                }
            }
        }
        VSwitchCommand::Create { name, vlan } => {
            if !approve(cli, &format!("create vSwitch \"{name}\" (vlan {vlan})"))? {
                return Ok(());
            }
            let vswitch = api::vswitch::create(client, name, *vlan)?;
            if cli.json {
                print_json(&vswitch)?;
            } else {
                println!("created vSwitch {} (vlan {})", vswitch.id, vswitch.vlan);
            }
        }
        VSwitchCommand::Connect { id, servers } => {
            if !approve(
                cli,
                &format!("connect {} server(s) to vSwitch {id}", servers.len()),
            )? {
                return Ok(());
            }
            api::vswitch::connect(client, *id, servers)?;
            println!("connected {} server(s) to vSwitch {id}", servers.len());
        }
        VSwitchCommand::Disconnect { id, servers } => {
            if !approve(
                cli,
                &format!("disconnect {} server(s) from vSwitch {id}", servers.len()),
            )? {
                return Ok(());
            }
            api::vswitch::disconnect(client, *id, servers)?;
            println!("disconnected {} server(s) from vSwitch {id}", servers.len());
        }
        VSwitchCommand::Cancel { id } => {
            if !approve(cli, &format!("cancel vSwitch {id}"))? {
                return Ok(());
            }
            api::vswitch::cancel(client, *id)?;
            println!("cancelled vSwitch {id}");
        }
    }
    Ok(())
}

fn print_server_table(servers: &[api::server::Server]) {
    if servers.is_empty() {
        println!("no servers");
        return;
    }
    for server in servers {
        println!(
            "{:<8} {:<16} {}",
            server.server_number, server.server_ip, server.server_name
        );
    }
}

fn print_server(server: &api::server::Server) {
    println!("number:  {}", server.server_number);
    println!("name:    {}", server.server_name);
    println!("ip:      {}", server.server_ip);
    if let Some(net) = &server.server_ipv6_net {
        println!("ipv6:    {net}");
    }
    if let Some(product) = &server.product {
        println!("product: {product}");
    }
    if let Some(status) = &server.status {
        println!("status:  {status}");
    }
    if let Some(dc) = &server.dc {
        println!("dc:      {dc}");
    }
}

fn print_failover_entry(entry: &api::failover::Failover) {
    match &entry.active_server_ip {
        Some(target) => println!("{} -> {target}", entry.ip),
        None => println!("{} -> (unrouted)", entry.ip),
    }
}

fn print_rescue(rescue: &api::boot::Rescue) {
    println!("server:   {}", rescue.server_number);
    if let Some(os) = &rescue.os {
        println!("os:       {os}");
    }
    if let Some(arch) = rescue.arch {
        println!("arch:     {arch}");
    }
    if let Some(active) = rescue.active {
        println!("active:   {active}");
    }
    if let Some(password) = &rescue.password {
        println!("password: {password}");
    }
}

fn print_rdns(entry: &api::rdns::Rdns) {
    println!("ip:  {}", entry.ip);
    match &entry.ptr {
        Some(ptr) => println!("ptr: {ptr}"),
        None => println!("ptr: (none)"),
    }
}

fn print_json<V: serde::Serialize>(value: &V) -> Result<()> {
    let encoded =
        serde_json::to_string_pretty(value).map_err(|error| Error::Decode(error.to_string()))?;
    println!("{encoded}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{decide, Decision};

    #[test]
    fn dry_run_always_wins() {
        assert_eq!(decide(true, false, true, None), Decision::DryRun);
        assert_eq!(decide(true, true, true, Some("y")), Decision::DryRun);
        assert_eq!(decide(true, false, false, None), Decision::DryRun);
    }

    #[test]
    fn yes_skips_the_prompt() {
        assert_eq!(decide(false, true, false, None), Decision::Proceed);
        assert_eq!(decide(false, true, true, None), Decision::Proceed);
    }

    #[test]
    fn non_interactive_without_yes_is_refused() {
        assert_eq!(decide(false, false, false, None), Decision::Refused);
        // A non-interactive session is refused even if an answer is supplied.
        assert_eq!(decide(false, false, false, Some("y")), Decision::Refused);
    }

    #[test]
    fn interactive_requires_an_affirmative_answer() {
        assert_eq!(decide(false, false, true, Some("y\n")), Decision::Proceed);
        assert_eq!(decide(false, false, true, Some("YES")), Decision::Proceed);
        assert_eq!(decide(false, false, true, Some("n")), Decision::Refused);
        assert_eq!(decide(false, false, true, Some("")), Decision::Refused);
        assert_eq!(decide(false, false, true, None), Decision::Refused);
    }
}
