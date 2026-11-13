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
  changed between runs" is unreachable *in v2.3*. v1 does answer it, per asset,
  via `with_logs=true` — see the note below on the `glpi_logs` grant.

Both live in the database.

**Decided on 2026-11-13, reversing what this section used to say.** The app
reads the GLPI database directly, and not through a scoped grant: the account
holds `ALL PRIVILEGES ON *.* ... WITH GRANT OPTION`, which is root-equivalent
on this server. This section previously said "still not decided for those two
— do not quietly reach for SQL"; that restriction is lifted, and the reasoning
it rested on is not, so both halves are recorded here.

What the reversal buys: the two gaps below stop being gaps, fleet-wide history
becomes one query instead of one per machine, and the next question of this
shape does not need a new grant and a new decision before it can be answered.

What it costs, unchanged from the argument against it:

- **Entity permissions are bypassed.** A profile scopes what the API returns;
  a `SELECT` sees every entity. Anything read this way must not be presented
  as though it had been filtered.
- **The schema is now an interface.** A column renamed upstream breaks a query
  at runtime, where the API would have dropped a field and left an `Option`
  blank. Treat every direct read as needing the same per-column care
  `glpi_refusedequipments` already gets — see the `db-read` tab.
- **The API stays the default.** The rule is not "SQL is now fine", it is
  "SQL is available where the API cannot answer". Where v2.3 or v1 answers a
  question, use it: same account, same profile rights, same entity scope.
- **Read-only is now a convention, not a constraint.** The grant permits
  `INSERT`, `UPDATE`, `DELETE` and DDL on every database, so nothing in the
  database refuses a write any more. This app reads and must keep reading:
  GLPI maintains its own history, entity scoping and derived state, and
  writing behind it corrupts the install in ways the UI will not show. The
  grant no longer enforces that — the code and the reviewer do.

The grant is to `pde@localhost` authenticating through the `unix_socket`
plugin, which is the identity the service already connects as — the unit is a
user unit and `GLPI_DB_URL` is `mysql://pde@localhost/glpi`, unchanged by any
of this. The two narrow grants on `glpi_logs` and `glpi_refusedequipments`
still appear in `SHOW GRANTS` and are now redundant.

What the socket plugin still gives: no password exists anywhere, and the
account cannot authenticate over TCP at all, because the plugin needs a peer
credential the kernel supplies only over a local socket. That is the whole of
the remaining protection, and it is about *reachability*, not authority.

What it does not give is any scoping. The grant is **not scoped to this app** —
it belongs to the login account, so every process running as `pde` is a DBA:
any shell, any editor plugin, any agent. `DROP DATABASE glpi` needs no
password, and `mysql.global_priv` (every account's authentication string) is
readable. Scoping it to the service would mean a separate Unix user and a
system unit; that has not been done, and until it is, "who may write to GLPI's
database" is answered by "whoever is running as pde".

What *has* been settled, in `docs/gl-db-api.md`: a database account does not
help with anything about the API itself. The endpoint list, each endpoint's
field list, the right → (tab, group) taxonomy, and which right an endpoint
requires are all PHP, not rows. And where a question *is* answerable outside
v2.3, the legacy v1 API answers it at lower cost — same account, same profile
rights, same entity scope, none of which direct SQL preserves. That is still
the reason to prefer v1 over a query wherever v1 answers; what changed above is
only that a query is now available when it does not.

### The `glpi_logs` grant is provisioning. Do not revoke it as a tidy-up.

**Superseded on 2026-11-13 as to its scope** — the account now holds all
privileges server-wide, so there is no longer a two-table grant to audit or
revoke.
The argument is kept because the reasoning is what matters and it generalises:
it is the worked example of the rule at the end of this section, and the same
mistake is now available against any table nothing reads yet.

The grant used to be `SELECT` on `glpi_refusedequipments` **and** `glpi_logs`.
Only the first was read by any code, so an audit by present usage concluded the
second was cruft. **That conclusion is wrong**, and it had already been reached
once and argued for in detail before being caught.

It is held for field-level change history — "what changed between runs". A
serial or MAC that silently changed between inventories is the natural fourth
quiet failure beside "stopped reporting", "never landed" and "no agent", and
it is squarely what this app is for.

**Correcting the reason first given for it, which was wrong.** This note used
to say the history is unreachable by *any* API at *any* version. It is not.
v2.3 models no `Log` schema at any path, so the API this app prefers cannot
answer it — but **v1 can**: `getItem` takes `with_logs=true` and returns that
asset's `glpi_logs` rows, gated by the `logs` right, which this account
already holds. Measured on this install: `Computer/1?with_logs=true` returns
42 entries, 7 of them field-level with real `old_value` → `new_value` pairs.

The gap is narrower and still real: v1 answers **one asset per request**. That
is the same per-machine cost that sent Hardware, Software and Network to v1
*collections* rather than per-asset calls, and on a fleet it is the difference
between one query and one per machine. That — fleet-wide history in a single
query — is what the grant buys, and it is the justification to keep.

More generally, and this is the part worth carrying beyond this one grant:
**this app is still being built, so judging anything by what the code
currently uses is the wrong test.** The right question is whether the
capability is on the way. The same error was made about GLPI 12's new API
surface before it was made about this grant.

## Why this app speaks two GLPI APIs

v2.3 for the asset-level view — agents, computers, entities, session;
**v1 (`/apirest.php`) for rights and authorizations, and for the three
inventory detail pages**, in `glpi/legacy.rs` and `glpi/inventory.rs`.

### v1 for rights and authorizations

Verified by reading `src/Glpi/Api/HL/`, not assumed:

- **`Profile_User` has zero occurrences anywhere under HL.** Who may use which
  profile, on which entity, and whether that is recursive, is not modelled.
- **HL's `Profile` schema carries neither `interface` nor any rights** — only
  `id`, `name`, `comment`, `is_default`, `helpdesk_hardware`,
  `helpdesk_item_type`, `managed_domainrecordtypes`, `mfa_enforced`.

So v2.3 can describe the session it is already in and nothing else: it can say
"something is missing" but never "this profile, on this entity, is the one to
fix". That question is the whole point of the Status and Rights pages.

This is a different trade from the MariaDB one above and must not be confused
with it. **v1 does not bypass GLPI's permissions** — every call is still made
as the service account and still filtered by its profile, which is the precise
objection to going direct to SQL. What it does cost is stability: GLPI calls v1
"closely tied to the GLPI source code ... no promise of stability between
versions", and it needs its own authentication because it rejects the OAuth
bearer outright.

### v1 for hardware, software and network

The three inventory pages under `/inv` are fleet-wide and read v1 collections.
Verified by reading `src/Glpi/Api/HL/Controller/` and by probing the running
install, not assumed:

- **Components, volumes, OS and software installs exist in v2.3 only beneath a
  single asset id** — `/Assets/Computer/{id}/Volume`,
  `/Assets/{itemtype}/{id}/Component/{type}`. There is no collection route for
  any of them: `Volume` and `Assets/Computer/Volume` both answer 404. A
  fleet-wide view would cost ~10 requests per machine; v1 answers each for the
  whole estate in one.
- **`NetworkPort` has a v2.3 schema but no route at any path**, and
  `IPAddress` has zero occurrences anywhere under HL. The Network page cannot
  be built on v2.3 at all — the nearest it offers is a MAC per network card,
  one request per machine, with no addresses attached.
- **A v2.3 software install names only its version** (`"1.33.7"`), never its
  title, and versions are served only under `/Assets/Software/{id}/Version` —
  about 1330 requests on this install. v1's `SoftwareVersion` collection
  carries `softwares_id`, so the whole mapping is one call.

`Legacy::get_paged` walks these; `Legacy::get` (one generous range, refuses
anything partial) is still what the bounded rights tables use. A single NixOS
machine holds 1468 `Item_SoftwareVersion` rows, so no fixed range is
defensible for inventory.

Scope comes from v2.3 regardless: every inventory page is filtered to the
computers `Assets/Computer` returns, so all pages mean the same thing by
"every machine" even though they read different APIs.

### The rule

Do not reach for v1 for anything v2.3 already answers. The full reasoning, the
endpoint-to-right mapping and fifteen traps are in `docs/gl-rights.md`.
`just probe <path>` hits v2.3; `just probe-v1 <path>` hits v1.

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
