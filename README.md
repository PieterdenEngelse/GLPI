# GLPI on NixOS

Two halves of one setup, built and run on a single machine (NixOS 26.05):

| | what |
|---|---|
| **the server** | a hand-written NixOS package and module for [GLPI](https://glpi-project.org/) 11.0.12 — IT asset management and service desk — plus a module for the inventory agent. nixpkgs has neither. |
| **`glpi-inv`** | a dashboard answering the one question GLPI cannot: which inventory agents have stopped reporting? |

They are independent: the modules run GLPI, the dashboard reads it over the
API. Either works without the other. They live together because debugging one
almost always means looking at the other.

```
glpi.nix  glpi-agent.nix  CLIENTS.md              the server
backend/  frontend/  tools/  systemd/             the dashboard
docs/                                             the written-up research
```

---

# The server

nixpkgs has **no `glpi` package and no `services.glpi` module**. It does have
`pkgs.glpi-agent` but **no `services.glpi-agent`**, only
`services.fusionInventory` for the abandoned predecessor. Both modules here
are written from scratch.

Everything was built and run on one machine (NixOS 26.05). `docs/gl-net.md` records
what was verified by executing it and what was not, and documents eight traps
that cost real debugging time — several of which produce a *working build* and
a broken system.

## Layout

| file | what |
|---|---|
| `glpi.nix` | the GLPI package + module: PHP env, nginx, MariaDB, optional LAN TLS, cron |
| `glpi-agent.nix` | the inventory agent module |
| `docs/gl-net.md` | working notes, design reasoning, and the traps |
| `docs/gl-client-serv.md` | the ordered setup manual, server through clients |
| `docs/gl-screens.md` | making and adjusting GLPI's own screens, and the capacity toggle that purges data |
| `docs/gl-adapter.md` | every route this dashboard uses into GLPI, and what each costs |
| `CLIENTS.md` | pointing Windows / macOS / other Linux clients at the server |

## Usage

Drop both files beside your `configuration.nix` and wire them in:

```nix
imports = [
  ./glpi.nix
  ./glpi-agent.nix
];

services.glpi.enable = true;

# Optional: serve to the LAN over HTTPS with a self-signed certificate,
# in addition to the plain-HTTP loopback listener. Opens the firewall port.
services.glpi.lan.enable = true;

# Optional: inventory this machine too.
services.glpi-agent = {
  enable = true;
  server = "https://${config.services.glpi.hostname}/";
  caCertFile = "${config.services.glpi.lan.tlsDir}/cert.pem";
};
```

`services.glpi.hostname` defaults to `${config.networking.hostName}.local`.
A `.local` name needs avahi to **publish**, which is a separate switch from
resolving — avahi answers for other hosts by default and never announces
itself:

```nix
services.avahi = {
  enable = true;
  nssmdns4 = true;
  openFirewall = true;
  publish = { enable = true; addresses = true; workstation = true; };
};
```

Then rebuild, and create the schema **once**:

```sh
sudo nixos-rebuild switch

sudo -u glpi glpi-console database:install \
  --db-name=glpi --db-user=glpi --db-password= --no-interaction
```

`--db-password=` with an **empty value** is required and is not a typo — the
account uses MariaDB's `unix_socket` plugin and has no password, but GLPI
validates the option and rejects a null one. See `docs/gl-net.md`, trap 5.

Native inventory ships **disabled**; the agent endpoint returns 403 until:

```sh
sudo -u glpi glpi-console config:set --context=inventory enabled_inventory 1
```

Then log in at `http://localhost:8080` as `glpi` / `glpi` and **change that
password** — GLPI ships four accounts with published defaults.

## Notable design points

- **GLPI's source stays read-only in `/nix/store`.** It normally writes inside
  its own tree; `SystemConfigurator.php` lets every path constant be set from
  the environment, so state goes to `/var/lib/glpi` instead.
- **The TLS key is generated at runtime, never in a derivation.** Nix store
  paths are world-readable, which is fine for a certificate and unacceptable
  for a private key.
- **Agents verify the self-signed certificate** via `--ca-cert-file` rather
  than skipping the check with `--no-ssl-check`, so a self-signed setup still
  gets authentication and not just encryption.
- **The agent's `environment` and `process` categories are excluded by
  default.** They collect environment variables *with values* (a routine place
  for credentials) and a full process list with command lines — neither tells
  you anything about the hardware or software installed.

## Status and scope

Verified by execution: the module, the loopback install, LAN HTTPS with
certificate verification, and one real agent run creating an asset.

**Not** verified: the Windows and macOS client instructions in `CLIENTS.md`.
They come from upstream documentation, with no such host available to test
them — `CLIENTS.md` marks exactly which parts those are, including one path it
could not confirm.

PHP is taken from the system's nixpkgs by default. If yours predates
**8.4.27**, read the CVE section of `docs/gl-net.md` first: two actively-exploited
interpreter RCEs affect every GLPI version equally, and no GLPI release fixes
them.

---

# glpi-inv — the dashboard

Answers one question GLPI itself cannot: **which inventory agents have
stopped reporting?**

GLPI shows the computer an agent created and keeps showing it, looking
perfectly healthy, long after that agent went silent. GLPI has no notion of an
agent being *overdue* — so a machine that stopped checking in weeks ago is
indistinguishable from one that reported this morning.

An axum backend reads GLPI's API; a Dioxus web UI presents it.

## What it shows

| page | question it answers |
|---|---|
| Agents | Which agents have stopped reporting? Most overdue first. |
| Computers | Which machines have **no agent at all** — assets nothing will ever update again. |
| Status | Who is this app in GLPI, and may it see everything? Profile, entity, recursion, and every authorization the account holds. |
| Rights | The full rights map of the active profile, grouped the way GLPI's own profile form groups it. |
| About | What the app judges, and the three things it cannot see. |

The last two exist because diagnosing a GLPI permission problem from outside
GLPI is genuinely hard: a refusal names no right. See `docs/gl-rights.md`.

## Status

Running against live GLPI, deployed as a systemd user unit on
`127.0.0.1:3200`, serving a dark/light themed UI built with Tailwind and
daisyUI.

- `nix develop --command just gate-rust` is the check — fmt, tests, and
  clippy under `-D warnings`. No test count is quoted here deliberately: the
  last one sat in this file until it was wrong by a factor of three.
- `glpi-cli` probes the API directly, bypassing the backend and the browser.

Credentials are yours to create once — see `docs/oauth-setup.md`.

## Two GLPI APIs

v2.3 (the High-Level API) for everything about inventory. v1 (`/apirest.php`)
**only** for rights and authorizations, because v2.3 does not model them at
all: `Profile_User` has zero occurrences in it, and its `Profile` schema
carries neither `interface` nor any rights. The trade and its traps are in
`CLAUDE.md` and `docs/gl-rights.md`.

## Layout

Follows [RERAG](https://github.com/PieterdenEngelse/RERAG)'s shape:

```
backend/            axum service + GLPI API client
frontend/fro/       Dioxus web UI
tools/glpi-cli/     probe the GLPI API from a terminal
scripts/            dump GLPI's right taxonomy (needs the glpi user)
systemd/            user unit
docs/               OAuth setup
Justfile            the one definition of every command
flake.nix           dev shell - this machine has no system-wide Rust
```

Note the two Nix roles in this repo: `flake.nix` is the **dev shell for the
dashboard**, while `glpi.nix` and `glpi-agent.nix` are **NixOS modules you
import** from a system configuration. They are unrelated.

## Quick start

```sh
nix develop --command just --list      # every command
nix develop --command just gate        # fmt + tests
nix develop --command just build       # backend
nix develop --command just run         # needs credentials, see below
nix develop --command just deploy      # install to ~/.local/lib + restart
```

The stylesheet is generated. `assets/styling/output.css` is committed so a
clone builds without npm, and `just css-build` regenerates it — the frontend
recipes depend on it, because class names live in the `.rs` sources and a
bundle shipping yesterday's CSS fails silently.

Credentials, once: `docs/oauth-setup.md`. Both GLPI APIs and the OAuth flow
ship **disabled**, and GLPI advertises its endpoints on the wrong port until
`url_base` is set — that document covers both.

## Why axum and not actix-web

RERAG is actix-web. This matches the GLPI engine and nixdash instead, so one
HTTP idiom covers all three. The *layout* is what is shared.

## What it cannot see

Three gaps, all in the API rather than in this code:

- **Refused submissions.** The HL API exposes `Agent` but **not**
  `RefusedEquipment`, so the most valuable failure mode — agent exits 0, GLPI
  rejects the submission, the asset never appears — is unreachable. Reading it
  needs database access, with the trade-offs in `CLAUDE.md`. That decision is
  open.
- **Field-level change history.** Also not exposed, so "what changed between
  runs" is out of reach too.
- **Agents outside the service account's entities.** GLPI filters every query
  by entity and the filtered rows leave no trace in the response, so
  completeness cannot be verified from inside. The dashboard states its scope
  on the page rather than implying there is none.

## Licence

The modules and the dashboard: MIT. GLPI itself is GPL-3.0-or-later and is not
vendored — `glpi.nix` fetches the upstream release tarball.
