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
    match &cli.command {
        Command::Server { command } => match command {
            ServerCommand::List => {
                let servers = api::server::list(&client)?;
                if cli.json {
                    print_json(&servers)?;
                } else {
                    print_server_table(&servers);
                }
            }
            ServerCommand::Get { number } => {
                let server = api::server::get(&client, *number)?;
                if cli.json {
                    print_json(&server)?;
                } else {
                    print_server(&server);
                }
            }
        },
        Command::Rdns { command } => match command {
            RdnsCommand::Get { ip } => {
                let entry = api::rdns::get(&client, ip)?;
                if cli.json {
                    print_json(&entry)?;
                } else {
                    print_rdns(&entry);
                }
            }
            RdnsCommand::Set { ip, ptr } => {
                let entry = api::rdns::set(&client, ip, ptr)?;
                if cli.json {
                    print_json(&entry)?;
                } else {
                    print_rdns(&entry);
                }
            }
        },
        Command::Reset { command } => match command {
            ResetCommand::Methods { number } => {
                let options = api::reset::options(&client, *number)?;
                if cli.json {
                    print_json(&options)?;
                } else {
                    println!("server:  {}", options.server_number);
                    println!("methods: {}", options.types.join(", "));
                }
            }
            ResetCommand::Run { number, kind } => {
                let result = api::reset::execute(&client, *number, (*kind).into())?;
                if cli.json {
                    print_json(&result)?;
                } else {
                    println!("server: {}", result.server_number);
                    println!("reset:  {}", result.kind);
                }
            }
        },
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
