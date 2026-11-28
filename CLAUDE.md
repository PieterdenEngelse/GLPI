# CLAUDE.md

Guidance for Claude Code working in this repository.

## What this is

**glpi-inv** answers one question GLPI itself cannot: *which inventory agents
have stopped reporting?* GLPI's asset list shows the computer an agent created
and goes on showing it forever, looking healthy, long after the agent went
quiet. Nothing in GLPI has a notion of an agent being overdue.

Layout follows `~/Projects/RERAG` — workspace with `backend/`,
`frontend/fro/`, `tools/`, plus `systemd/`, `scripts/`, `docs/`, and a
`Justfile` as the one definition of every command. The *stack* deliberately
differs: **axum, not actix-web**, matching the GLPI engine and nixdash so
there is one HTTP idiom across these services.

## Build and run

**There is no system-wide Rust on this machine** — `rustc`, `cargo`, `gcc` and
the rest were removed from `environment.systemPackages`. Everything goes
through the dev shell:

```sh
nix develop --command just --list
```

| recipe | what | cost |
|---|---|---|
| `just build` | backend only | seconds after the first |
| `just gate` | fmt check + tests | seconds |
| `just gate-rust` | the above plus clippy | slower |
| `just build-frontend` | wasm bundle via dx | ~7 min cold |
| `just dev-frontend` | dx dev server, hot reload | — |
| `just run` | run the backend | — |
| `just probe <path>` | hit GLPI's API directly | — |

`cargo build` at the root builds **only** the backend (`default-members`).
The frontend is a wasm target; building it as part of a host-target workspace
build tries to link wasm into a native binary.

## Rules, in force whether or not the file behind them was read

- **v2.3 first, then v1, then SQL.** Never v1 for what v2.3 answers, never SQL
  for what either answers. A direct read bypasses entity permissions, so never
  present one as though it had been filtered.
- **Never write a table GLPI manages** — its history, entity scoping and
  derived state all break invisibly. Go through the API or GLPI's own code.
  Tables it does not manage (`glpi_plugin_*`, this app's `glpi_inv`) are fair
  game.
- **Each write path gets its own new `GLPI_INV_*` flag**, off by default.
  Never widen an existing one. The `glpi_inv` sampler is the one exception.
- **Never judge a capability by what the code uses today.** This app is still
  being built; auditing by present usage has been wrong twice.
- **Verify against the GLPI source or this install, never from memory.**

## Where the rest lives

Read the file named before working on its area; each is self-contained.

| file in `docs/claude/` | read it before |
|---|---|
| `toolchain-pins.md` | touching `flake.nix`, `dioxus`, `dx`, `wasm-bindgen` |
| `db-access.md` | any direct SQL, or what the GLPI API cannot give |
| `two-apis.md` | adding an endpoint, or picking v2.3 vs v1 |
| `writes.md` | every write path and its flag; the `glpi_inv` sampler |
| `php-tuning.md` | session expiry, php.ini, the Logout tile |
| `mariadb-variables.md` | the variables pages, `hazard`, the init file |
| `db-restart.md` | `scripts/db-restart.sh`, `backend/src/db_restart.rs` |
| `engine-page.md` | before even considering a storage-engine page or write |

Reference docs, not guidance: `docs/gl-rights.md` (endpoint-to-right mapping
and fifteen traps), `docs/gl-db-api.md` (what a DB account does not buy),
`~/docs/gl-win-install.md` (restart and validation, per OS).

## Conventions

- **Field names mirror GLPI's schema exactly** (`deviceid`, not `device_id`),
  so this code can be diffed against the GLPI source. Do not prettify them.
- **An unreachable upstream must never render as an empty list.** An empty
  agent list and a broken connection mean opposite things; the frontend has
  distinct states for both, and the backend returns 502 with GLPI's own
  message rather than swallowing it.
- **Timestamps from GLPI are local time with no offset.** Parsing them as UTC
  shifts every reading by the machine's offset and silently mislabels a whole
  fleet. There is a test for this.
- **Staleness defaults to 26 hours, not 24.** The agent timer runs daily with
  a `RandomizedDelaySec`, so 24 flags healthy machines whose run drifted.
- **Credentials come from the environment only.** Never a file in the repo,
  never anything Nix copies — store paths are world-readable, so a secret in
  a derivation is exposed locally before anything is ever pushed.
- The frontend's `AgentStatus` **duplicates** the backend's deliberately
  (RERAG's layout has no shared crate). Every field is `Option` so a drifting
  backend degrades to a blank cell rather than a failed parse. Change the two
  together.
