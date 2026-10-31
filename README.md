# glpi-inv

Answers one question GLPI itself cannot: **which inventory agents have
stopped reporting?**

GLPI shows the computer an agent created and keeps showing it, looking
perfectly healthy, long after that agent went silent. GLPI has no notion of an
agent being *overdue* — so a machine that stopped checking in weeks ago is
indistinguishable from one that reported this morning.

An axum backend reads agent telemetry from GLPI's High-Level API; a Dioxus web
UI lists agents most-overdue first.

## Status

Scaffold, building and tested. The agent view is implemented end to end;
it has not yet been run against live GLPI data, because that needs an OAuth
client you create — see `docs/oauth-setup.md`.

- backend compiles, 5 tests pass
- frontend compiles and bundles to wasm
- `glpi-cli` probes the API directly

## Layout

Follows [RERAG](https://github.com/PieterdenEngelse/RERAG)'s shape:

```
backend/            axum service + GLPI API client
frontend/fro/       Dioxus web UI
tools/glpi-cli/     probe the GLPI API from a terminal
systemd/            user unit
docs/               OAuth setup
Justfile            the one definition of every command
flake.nix           dev shell - this machine has no system-wide Rust
```

## Quick start

```sh
nix develop --command just --list      # every command
nix develop --command just gate        # fmt + tests
nix develop --command just build       # backend
nix develop --command just run         # needs credentials, see below
```

Credentials, once: `docs/oauth-setup.md`. Both GLPI APIs and the OAuth flow
ship **disabled**, and GLPI advertises its endpoints on the wrong port until
`url_base` is set — that document covers both.

## Why axum and not actix-web

RERAG is actix-web. This matches the GLPI engine and nixdash instead, so one
HTTP idiom covers all three. The *layout* is what is shared.

## Known gap

GLPI's HL API exposes `Agent` but **not** `RefusedEquipment`, and no
field-level change history. So the most valuable failure mode — agent exits 0,
GLPI rejects the submission, asset never appears — is not reachable through
the API. Reading it needs database access, with the trade-offs in `CLAUDE.md`.
That decision is open.

## Licence

MIT.
