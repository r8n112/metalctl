# Backlog

Self-contained tasks for `metalctl`, ordered by priority. Pick the first task
whose status is `todo`, satisfy every acceptance criterion, then mark it `done`.

Already implemented: `server list/get`, `rdns get/set`, `reset methods/run`,
`boot rescue` (get/activate/deactivate), `failover list/get/route`, `traffic`
query, `vswitch list/get/create/connect/disconnect/cancel`.

## T001 — SSH key endpoints

- Status: todo
- Depends on: none

Implement `GET /key`, `GET /key/{fingerprint}`, `POST /key`, `POST /key/{...}`
(name update), `DELETE /key/{fingerprint}` in `src/api/key.rs`.

Acceptance criteria:
- [ ] a failing-first test parses a representative key response
- [ ] create/update send the documented form fields; delete is tested
- [ ] CLI: `metalctl key list|get|create|delete`
- [ ] `just ci` is green

## T002 — Subnet endpoints

- Status: todo
- Depends on: none

Implement `GET /subnet`, `GET /subnet/{ip}`, `POST /subnet/{ip}` (traffic
warning / cancellation) in `src/api/subnet.rs`.

Acceptance criteria:
- [ ] list (array of envelopes) and get are tested
- [ ] typed model documented
- [ ] `just ci` is green

## T003 — Wake-on-LAN

- Status: todo
- Depends on: none

Implement `GET /wol/{server-number}` and `POST /wol/{server-number}`.

Acceptance criteria:
- [ ] get and send are both covered by failing-first tests
- [ ] CLI: `metalctl wol get|send <server-number>`
- [ ] `just ci` is green

## T004 — Ordering / cancellation endpoints

- Status: todo
- Depends on: none

Implement the transactional ordering endpoints under `/order` for at least one
resource type, including the confirmation flow.

Acceptance criteria:
- [ ] request + confirmation flow covered by tests with the mock transport
- [ ] errors surfaced as `Error::Api` with the API message
- [ ] `just ci` is green

## T005 — Firewall (Robot) endpoints

- Status: todo
- Depends on: none

Implement the Robot firewall endpoints (`/firewall`, `/firewall/template`) in
`src/api/firewall.rs`. Note the API can express multiple comma-delimited ports
per rule.

Acceptance criteria:
- [ ] list/get/template operations covered by tests
- [ ] typed rule model documented
- [ ] `just ci` is green

## T006 — Timeouts and bounded retry

- Status: todo
- Depends on: none

`UreqTransport` currently has no timeout. Add a configurable request timeout and
a single bounded retry for transient transport errors. The Robot API also rate
limits; surface 429 as `Error::Api` with a clear message.

Acceptance criteria:
- [ ] `UreqTransport` accepts a timeout configuration
- [ ] retry behaviour proven with the mock transport
- [ ] 429 handling documented
- [ ] `just ci` is green

## T007 — Ergonomic id newtypes

- Status: todo
- Depends on: none

`u32` server numbers and VLAN ids are easy to confuse. Introduce `ServerNumber`
and `VlanId` newtypes in the public API without breaking the CLI. (Coordinate
with T001–T005 so modules use the newtypes.)

Acceptance criteria:
- [ ] newtypes have `From`/`Into` for their primitive and `Display`
- [ ] public functions use the newtypes
- [ ] `just ci` is green

## T008 — MSRV CI job

- Status: todo
- Depends on: none

The manifest declares `rust-version = "1.74"`. Add a CI job that builds with the
declared MSRV so it cannot silently drift.

Acceptance criteria:
- [ ] CI has an MSRV job pinned to the version in `Cargo.toml`
- [ ] the job passes on `main`

## T009 — Uniform `--json` and examples

- Status: todo
- Depends on: none

Ensure every CLI command honours the global `--json` flag, and add a runnable
example per command group under `examples/`.

Acceptance criteria:
- [ ] all commands produce valid JSON with `--json`
- [ ] at least two compiling examples exist
- [ ] `just ci` is green
