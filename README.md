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
- **Bounded, idempotency-aware requests.** `UreqTransport` enforces
  connect/read/write timeouts (default 30s). `RobotClient` retries transient
  transport errors on idempotent `GET`s with exponential backoff, but **never**
  retries `POST`/`DELETE`, which may have applied even when the response is
  lost. HTTP 429 surfaces as `Error::RateLimited`.
- **No panics on input.** Errors are typed and actionable; credentials are
  redacted in `Debug` output.
- **Safe destructive operations.** Mutating commands never run unless confirmed.
- **Documented public API** with compiling examples.

## Usage

```sh
export HETZNER_ROBOT_USER=...
export HETZNER_ROBOT_PASSWORD=...

cargo run -- server list
cargo run -- server get 321
cargo run -- --json server list
```

### Credentials

Credentials are resolved with the precedence **CLI flags → environment → config
file**:

- flags: `--user <USER>` and `--password-file <PATH>` (the password is read from
  a file, never passed as a plain argument);
- environment: `HETZNER_ROBOT_USER` and `HETZNER_ROBOT_PASSWORD`;
- config file: `$METALCTL_CONFIG`, else `$XDG_CONFIG_HOME/metalctl/config.toml`,
  else `~/.config/metalctl/config.toml` — a TOML file with `user = "..."` and
  `password = "..."`. Unknown keys and malformed TOML are rejected. On Unix the
  file must be owner-only (`chmod 600`); broader permissions are refused.
  Passwords are never written to logs, errors, or `--json` output.

### Destructive operations

`rdns set`, `reset run`, `boot rescue activate`/`deactivate`, `failover route`,
and every mutating `vswitch` command require confirmation:

- interactively (stdin is a terminal) you are prompted `[y/N]`;
- in a non-interactive session (piped stdin, cron, CI) the command **refuses**
  unless `--yes` is given, so nothing runs just because stdin was unavailable;
- `--dry-run` prints what would happen and exits **without any network call**
  and **without requiring credentials** (nothing is sent).

```sh
metalctl reset run 321 --type power --dry-run   # print only
metalctl reset run 321 --type power --yes       # non-interactive, confirmed
```

As a library:

```rust
use metalctl::{api, Credentials, RobotClient};

let client = RobotClient::new(Credentials::from_env()?);
for server in api::server::list(&client)? {
    println!("{} {}", server.server_number, server.server_name);
}
```

## MCP server (`metalctl-mcp`)

The repository is a Cargo workspace: the `metalctl` library/CLI plus
`crates/metalctl-mcp`, a [Model Context Protocol](https://modelcontextprotocol.io)
server exposing every command as a tool over stdio.

```sh
cargo build -p metalctl-mcp
cargo install --path crates/metalctl-mcp   # optional, installs `metalctl-mcp`
```

Register it with an MCP client (opencode example):

```json
{
  "mcp": {
    "metalctl": {
      "type": "local",
      "command": ["metalctl-mcp"],
      "enabled": true,
      "environment": {
        "HETZNER_ROBOT_USER": "{env:HETZNER_ROBOT_USER}",
        "HETZNER_ROBOT_PASSWORD": "{env:HETZNER_ROBOT_PASSWORD}"
      }
    }
  }
}
```

Tools advertise MCP annotations (`readOnlyHint` for reads, `destructiveHint` for
mutations) so clients can reason about safety. Read-only tools run directly;
destructive tools (`reset_run`, `rdns_set`, `failover_route`, `vswitch_cancel`,
the rescue-system changes, ...) refuse to run unless called with
`confirm = true`, so an agent cannot mutate a server by
accident.

## Development

```sh
just fmt        # rustfmt + taplo
just lint       # clippy, warnings denied
just test       # nextest + doctests
just audit      # cargo-deny (advisories/licenses/bans/sources)
just ci         # everything, as CI runs it
```

Toolchain: stable (`rust-toolchain.toml`). MSRV is **Rust 1.88** (dictated by
`rmcp` in the `metalctl-mcp` member; CI enforces it). Commands operate on the
whole workspace, so pass `-p metalctl` for just the library/CLI.

## License

MIT OR Apache-2.0.
