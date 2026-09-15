# AGENTS.md — operating instructions for automated agents

This repository follows the r8n112 engineering standard. If you are an
autonomous coding agent, read this file in full before making any change.

## What this project is

`metalctl` is a low-level Rust library and CLI for the **Hetzner Robot**
(bare-metal) API. Look at `BACKLOG.md` for what is still missing; that file is
the authoritative task list.

## Non-negotiables

- **TDD.** Write a failing test first, make it pass with the smallest change,
  then refactor.
- **No `unsafe`** (`unsafe_code = "forbid"`); do not weaken that.
- **No panics on input.** Never `unwrap`/`expect`/`panic!` on network or user
  input. Use the typed `Error`; API failures become `Error::Api`.
- **Tests never touch the network.** Implement `Transport` with an in-memory
  mock (see `tests/robot_client.rs`). Production uses `UreqTransport`.
- **Document the public API.** Every public item has a doc comment; fallible
  functions document `# Errors`.
- **Do not weaken lints** in `Cargo.toml`; justify any `#[allow(...)]` inline.
- **Minimal dependencies.** New deps must be justified; `cargo-deny` enforces
  the license allow-list (it already allows `CDLA-Permissive-2.0` for
  `webpki-roots`).
- **Small signed commits.** `git commit -s`, one logical change, conventional
  message.

## Commands

```sh
just fmt    # rustfmt + taplo
just lint   # clippy --all-targets --all-features -- -D warnings
just test   # tests + doctests
just audit  # cargo-deny
just ci     # everything, as CI runs it
```

The crate targets stable Rust (`rust-toolchain.toml`). If stable is not
installed locally, prefix commands with `RUSTUP_TOOLCHAIN=nightly`.

## Architecture

- `src/lib.rs` — crate root, public re-exports.
- `src/client.rs` — `RobotClient<T>`; `get_json`, `post_form`, `post_form_ok`,
  `delete`, `delete_form`; base-URL normalisation; form encoding (keeps `[]`
  literal for PHP-style array parameters).
- `src/transport.rs` — `Transport` trait, `HttpRequest`/`HttpResponse`,
  `UreqTransport` (supports GET/POST/DELETE).
- `src/credentials.rs` — `Credentials` (env `HETZNER_ROBOT_USER`,
  `HETZNER_ROBOT_PASSWORD`), password redacted in `Debug`.
- `src/error.rs` — typed `Error`.
- `src/api/*.rs` — one module per endpoint group: `server`, `rdns`, `reset`,
  `boot`, `failover`, `traffic`, `vswitch`.
- `src/main.rs` — `clap` CLI; one handler function per command group.

## Robot API conventions

- Most endpoints wrap the payload in an outer object, e.g. `{"server": {...}}`;
  model it with a small private `Envelope<T>` struct (see `api/server.rs`).
- **vSwitch is the exception:** it returns bare arrays/objects (see
  `api/vswitch.rs`).
- `GET /server` returns an array of envelopes; `GET /server/{n}` a single one.
- Forms use `application/x-www-form-urlencoded`; array parameters are written
  as `key[]`.

## How to add an endpoint

1. Pick a task from `BACKLOG.md`; note its acceptance criteria.
2. Add a failing integration test in `tests/robot_client.rs` asserting **both**
   the parsed result and the exact HTTP request (method, URL, body).
3. Implement in a new or existing `src/api/<group>.rs`; add a typed model with
   `serde` and document every public item.
4. Re-export nothing new from `lib.rs` unless needed; wire the CLI in
   `src/main.rs` (add a command enum + a handler function; keep each function
   under 100 lines or clippy will reject it).
5. `just ci` until green, then commit `feat(<scope>): ...`.

## Definition of done

- [ ] A test was added first and now passes.
- [ ] `cargo fmt --check`, clippy, tests and docs all pass.
- [ ] Public items documented; no new `unwrap`/`expect` on input.
- [ ] `cargo-deny` clean; no unjustified dependency.
- [ ] Commit is signed, conventional, and references the backlog task id.

## Backlog

`BACKLOG.md` lists self-contained tasks with acceptance criteria. Work them in
order unless told otherwise.
