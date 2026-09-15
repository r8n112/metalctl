//! `metalctl` command-line interface.

use clap::{Parser, Subcommand};
use metalctl::{api, Credentials, Error, Result, RobotClient};

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

fn run(cli: &Cli) -> Result<()> {
    let client = RobotClient::new(Credentials::from_env()?);
    dispatch(&client, cli)
}

fn dispatch(client: &RobotClient, cli: &Cli) -> Result<()> {
    match &cli.command {
        Command::Server { command } => server_command(client, cli.json, command),
        Command::Rdns { command } => rdns_command(client, cli.json, command),
        Command::Reset { command } => reset_command(client, cli.json, command),
        Command::Boot { command } => boot_command(client, cli.json, command),
        Command::Failover { command } => failover_command(client, cli.json, command),
        Command::Traffic {
            kind,
            from,
            to,
            ips,
        } => traffic_command(client, cli.json, kind, from, to, ips),
        Command::Vswitch { command } => vswitch_command(client, cli.json, command),
    }
}

fn server_command(client: &RobotClient, json: bool, command: &ServerCommand) -> Result<()> {
    match command {
        ServerCommand::List => {
            let servers = api::server::list(client)?;
            if json {
                print_json(&servers)?;
            } else {
                print_server_table(&servers);
            }
        }
        ServerCommand::Get { number } => {
            let server = api::server::get(client, *number)?;
            if json {
                print_json(&server)?;
            } else {
                print_server(&server);
            }
        }
    }
    Ok(())
}

fn rdns_command(client: &RobotClient, json: bool, command: &RdnsCommand) -> Result<()> {
    match command {
        RdnsCommand::Get { ip } => {
            let entry = api::rdns::get(client, ip)?;
            if json {
                print_json(&entry)?;
            } else {
                print_rdns(&entry);
            }
        }
        RdnsCommand::Set { ip, ptr } => {
            let entry = api::rdns::set(client, ip, ptr)?;
            if json {
                print_json(&entry)?;
            } else {
                print_rdns(&entry);
            }
        }
    }
    Ok(())
}

fn reset_command(client: &RobotClient, json: bool, command: &ResetCommand) -> Result<()> {
    match command {
        ResetCommand::Methods { number } => {
            let options = api::reset::options(client, *number)?;
            if json {
                print_json(&options)?;
            } else {
                println!("server:  {}", options.server_number);
                println!("methods: {}", options.types.join(", "));
            }
        }
        ResetCommand::Run { number, kind } => {
            let result = api::reset::execute(client, *number, (*kind).into())?;
            if json {
                print_json(&result)?;
            } else {
                println!("server: {}", result.server_number);
                println!("reset:  {}", result.kind);
            }
        }
    }
    Ok(())
}

fn boot_command(client: &RobotClient, json: bool, command: &BootCommand) -> Result<()> {
    match command {
        BootCommand::Rescue { command } => match command {
            RescueCommand::Get { number } => {
                let rescue = api::boot::rescue(client, *number)?;
                if json {
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
                let rescue =
                    api::boot::activate_rescue(client, *number, os, arch, authorized_keys)?;
                if json {
                    print_json(&rescue)?;
                } else {
                    print_rescue(&rescue);
                }
            }
            RescueCommand::Deactivate { number } => {
                api::boot::deactivate_rescue(client, *number)?;
                println!("rescue system deactivated for server {number}");
            }
        },
    }
    Ok(())
}

fn failover_command(client: &RobotClient, json: bool, command: &FailoverCommand) -> Result<()> {
    match command {
        FailoverCommand::List => {
            let entries = api::failover::list(client)?;
            if json {
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
            if json {
                print_json(&entry)?;
            } else {
                print_failover_entry(&entry);
            }
        }
        FailoverCommand::Route {
            ip,
            active_server_ip,
        } => {
            let entry = api::failover::route(client, ip, active_server_ip)?;
            if json {
                print_json(&entry)?;
            } else {
                print_failover_entry(&entry);
            }
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

fn traffic_command(
    client: &RobotClient,
    json: bool,
    kind: &str,
    from: &str,
    to: &str,
    ips: &[String],
) -> Result<()> {
    let traffic = api::traffic::query(client, kind, from, to, ips)?;
    if json {
        print_json(&traffic)?;
    } else {
        for (ip, buckets) in &traffic.data {
            let total: f64 = buckets.values().map(|statistic| statistic.total).sum();
            println!("{ip}: {total:.2} GiB");
        }
    }
    Ok(())
}

fn vswitch_command(client: &RobotClient, json: bool, command: &VSwitchCommand) -> Result<()> {
    match command {
        VSwitchCommand::List => {
            let switches = api::vswitch::list(client)?;
            if json {
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
            if json {
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
            let vswitch = api::vswitch::create(client, name, *vlan)?;
            if json {
                print_json(&vswitch)?;
            } else {
                println!("created vSwitch {} (vlan {})", vswitch.id, vswitch.vlan);
            }
        }
        VSwitchCommand::Connect { id, servers } => {
            api::vswitch::connect(client, *id, servers)?;
            println!("connected {} server(s) to vSwitch {id}", servers.len());
        }
        VSwitchCommand::Disconnect { id, servers } => {
            api::vswitch::disconnect(client, *id, servers)?;
            println!("disconnected {} server(s) from vSwitch {id}", servers.len());
        }
        VSwitchCommand::Cancel { id } => {
            api::vswitch::cancel(client, *id)?;
            println!("cancelled vSwitch {id}");
        }
    }
    Ok(())
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
