# metalctl

A small, synchronous Rust library and CLI for the **Hetzner Robot** (bare-metal)
API.

## Prior art and scope (read this first)

**[`hrobot-rs`](https://github.com/MathiasPius/hrobot-rs)** (`hrobot` on
crates.io, MIT) is a mature, **async** Robot client that covers **more** of the
API than `metalctl` does. If you want a full-featured Robot client, use `hrobot`.

`metalctl` does not aim to replace it or reach API parity. It deliberately
occupies a smaller niche:

- **synchronous**, no async runtime;
- **minimal dependencies** (`ureq` + `serde` + `thiserror` + `clap`);
- **CLI-first** — it ships the `metalctl` binary, which `hrobot` does not;
- **offline, deterministic tests** via an in-memory transport.

We are upfront that the endpoint modules overlap with `hrobot`. The reason for
building it is the CLI and the sync/minimal-deps posture, not coverage. See
[BACKLOG.md](BACKLOG.md) for where the effort goes next.

## Status

Early (`0.1.0`). Implemented: HTTP transport abstraction, credentials, error
taxonomy, `server list`/`get`, `rdns get`/`set`, `reset methods`/`run`,
`boot rescue get`/`activate`/`deactivate`, `failover list`/`get`/`route`,
`traffic` queries, and `vswitch list`/`get`/`create`/`connect`/`disconnect`/`cancel`.
Remaining Robot areas (keys, subnets, WOL, ordering) follow.

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
