# metalctl

A low-level Rust library and CLI for the **Hetzner Robot** (bare-metal) API.

Hetzner publishes official clients for the Cloud API but none for the Robot
API in Rust. `metalctl` fills that gap with a small, dependency-light crate and
a thin command-line interface.

## Status

Early (`0.1.0`). Implemented: HTTP transport abstraction, credentials, error
taxonomy, `server list`/`get`, and `rdns get`/`set`. More Robot endpoints
(reset, boot, failover, vSwitch, traffic) follow.

## Design

- **Testable by construction.** [`Transport`] is a trait; production uses
  `ureq`, tests inject an in-memory mock, so the whole suite runs offline.
- **Minimal dependencies.** `serde`/`serde_json`, `thiserror`, `base64`,
  `ureq`, `clap`. No async runtime.
- **No panics on input.** Errors are typed and actionable; credentials are
  redacted in `Debug` output.
- **Documented public API** with compiling examples.

## Usage

```sh
export HETZNER_ROBOT_USER=...
export HETZNER_ROBOT_PASSWORD=...

cargo run -- server list
cargo run -- server get 321
cargo run -- --json server list
```

As a library:

```rust
use metalctl::{api, Credentials, RobotClient};

let client = RobotClient::new(Credentials::from_env()?);
for server in api::server::list(&client)? {
    println!("{} {}", server.server_number, server.server_name);
}
```

## Development

```sh
just fmt        # rustfmt + taplo
just lint       # clippy, warnings denied
just test       # nextest + doctests
just audit      # cargo-deny (advisories/licenses/bans/sources)
just ci         # everything, as CI runs it
```

Toolchain: stable (`rust-toolchain.toml`). Targets stable Rust 1.74+.

## License

MIT OR Apache-2.0.
