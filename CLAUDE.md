# CLAUDE.md

Guidance for Claude Code working in this repository.

## What this is

**glpi-inv** answers one question GLPI itself cannot: *which inventory agents
have stopped reporting?* GLPI's asset list shows the computer an agent created
and goes on showing it forever, looking healthy, long after the agent went
quiet. There is no notion of an agent being overdue anywhere in GLPI.

Layout follows `~/Projects/RERAG` — workspace with `backend/`,
`frontend/fro/`, `tools/`, plus `systemd/`, `scripts/`, `docs/`, and a
`Justfile` as the one definition of every command. The *stack* deliberately
differs: **axum, not actix-web**, matching the GLPI engine and nixdash so
there is one HTTP idiom across these services.

## Build and run

**There is no system-wide Rust on this machine.** `rustc`, `cargo`, `gcc` and
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

## Three version pins that must move together

This cost real time. `dx`, the `dioxus` crate and `wasm-bindgen-cli` are
mutually version-locked:

1. **`dioxus` is pinned with `=0.7.9`**, not `"0.7"`. A caret range resolves
   to the newest 0.7.x and `dx` then refuses outright: *"dx and dioxus
   versions are incompatible: dx 0.7.9, dioxus 0.7.10"*.
2. **`wasm-bindgen-cli` is built at 0.2.130 in `flake.nix`**, because nixpkgs
   ships 0.2.121 and `dx` checks the CLI against the crate. Pinning the crate
   *down* is not possible: `js-sys 0.3.107` requires wasm-bindgen 0.2.130,
   `js-sys` arrives via `chrono` in the backend, and cargo resolves one
   version per workspace.
3. It uses **`pkgs.buildWasmBindgenCli`**, not `.override { version = ...; }`.
   The nixpkgs package is `wasm-bindgen-cli_0_2_121` and its function takes
   only `{ buildWasmBindgenCli, fetchCrate, rustPlatform }` — passing
   `version` fails with *"called with unexpected argument"*.

Bumping `dioxus-cli` in nixpkgs means revisiting all three.

Also: `fetchCrate` hashes the **unpacked** source, so a hash from
`nix store prefetch-file` on the crates.io download URL does not apply. Use a
placeholder and read the real hash from the mismatch.

## What the GLPI API cannot give us

Verified by reading `src/Glpi/Api/HL/Controller/InventoryController.php` in
GLPI 11.0.12, not assumed:

- **`Agent` IS exposed**, with `deviceid`, `last_contact`, `version`, `tag`,
  `remote_address`, `itemtype`/`items_id`. This is the app's core data.
- **`RefusedEquipment` is NOT exposed** — zero occurrences. That is the
  failure mode most worth surfacing: the agent exits 0, GLPI rejects the
  submission, the asset silently never appears, and nothing in the agent's
  own log says so.
- **No field-level change history** (`glpi_logs`) under Inventory, so "what
  changed between runs" is unreachable too.

Both live in the database. The options are a read-only MariaDB user scoped to
those two tables, or doing without. **That choice has not been made** — do not
quietly reach for SQL. Going direct to MariaDB bypasses GLPI's entity
permissions and couples this app to GLPI's schema across upgrades, which is
why the API is the default despite the gap.

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
