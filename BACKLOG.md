# Backlog

Self-contained tasks for `metalctl`, ordered by priority. Pick the first task
whose status is `todo`, satisfy every acceptance criterion, then mark it `done`.

**Positioning.** `hrobot-rs` already covers more of the Robot API, asynchronously.
`metalctl` is deliberately a sync, minimal-dependency, **CLI-first** alternative
(see README). Prefer tasks that improve the CLI, ergonomics, safety and the
sync/minimal posture over adding endpoints purely to match `hrobot`.

Already implemented: `server list/get`, `rdns get/set`, `reset methods/run`,
`boot rescue` (get/activate/deactivate), `failover list/get/route`, `traffic`
query, `vswitch list/get/create/connect/disconnect/cancel`.

## T001 — Shell completions and a man page

- Status: todo
- Depends on: none

Generate `bash`/`zsh`/`fish` completions and a man page from the `clap`
definition, so the CLI is usable without reading the source.

Acceptance criteria:
- [ ] a `metalctl completions <shell>` subcommand (or build script) emits completions
- [ ] a man page is generated or documented
- [ ] README documents installation of the completion
- [ ] `just ci` green

## T002 — Credential resolution beyond the environment

- Status: todo
- Depends on: none

Today credentials come only from `HETZNER_ROBOT_USER`/`HETZNER_ROBOT_PASSWORD`.
Add a documented resolution order (flags → env → config file) with the password
never logged and redacted in any error.

Acceptance criteria:
- [ ] `--user`/`--password-file` or an equivalent are supported
- [ ] precedence is documented and covered by tests
- [ ] the password never appears in `Debug`, errors, or `--json` output
- [ ] `just ci` green

## T003 — Uniform output formats

- Status: todo
- Depends on: none

Replace the boolean `--json` with `--output <table|json>` while keeping `--json`
as an alias. Ensure every command emits stable, scriptable output.

Acceptance criteria:
- [ ] every command supports both formats
- [ ] JSON output is documented as the scripting interface
- [ ] `just ci` green

## T004 — `--dry-run` for destructive commands

- Status: todo
- Depends on: none

`reset run`, `vswitch cancel`, and `boot rescue deactivate` act destructively.
Add `--dry-run` (and a confirmation prompt by default) that prints the request
that would be sent without sending it.

Acceptance criteria:
- [ ] `--dry-run` is covered by a test that asserts no request is sent
- [ ] non-interactive use is possible (for example `--yes`)
- [ ] `just ci` green

## T005 — Transport hardening: timeouts, retry, rate limits

- Status: todo
- Depends on: none

`UreqTransport` has no timeout. Add a configurable timeout, one bounded retry
for transient transport errors, and a clear message for HTTP 429 (the Robot API
rate-limits).

Acceptance criteria:
- [ ] `UreqTransport` accepts a timeout configuration
- [ ] retry is proven with the mock transport
- [ ] 429 surfaces a documented, distinct message
- [ ] `just ci` green

## T006 — CLI integration tests via `--base-url`

- Status: todo
- Depends on: T003

Add a hidden `--base-url` (or env override) so the binary can be pointed at a
local stub server, then add integration tests that run the compiled binary
against a `std::net::TcpListener` stub.

Acceptance criteria:
- [ ] the binary honours a configurable base URL
- [ ] at least one end-to-end CLI test runs offline against a local stub
- [ ] `just ci` green

## T007 — MSRV CI job

- Status: todo
- Depends on: none

The manifest declares `rust-version = "1.74"`. Add a CI job that builds with the
declared MSRV so it cannot silently drift.

Acceptance criteria:
- [ ] CI has an MSRV job pinned to the version in `Cargo.toml`
- [ ] the job passes on `main`

## T008 — Endpoints only where the CLI needs them

- Status: todo
- Depends on: T003

Add `keys`, `subnet` and `wol` support **only** to the extent the CLI needs it
for common bare-metal workflows. This is not an attempt to match `hrobot`'s
coverage.

Acceptance criteria:
- [ ] each added endpoint has a mocked request/parse test
- [ ] the README's "Prior art and scope" section remains accurate
- [ ] `just ci` green
