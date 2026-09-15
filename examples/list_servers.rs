//! Lists all dedicated servers using credentials from the environment.
//!
//! ```sh
//! HETZNER_ROBOT_USER=... HETZNER_ROBOT_PASSWORD=... cargo run --example list_servers
//! ```

use metalctl::{api, Credentials, RobotClient};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = RobotClient::new(Credentials::from_env()?);
    for server in api::server::list(&client)? {
        println!(
            "{}  {}  {}",
            server.server_number, server.server_ip, server.server_name
        );
    }
    Ok(())
}
