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
- **Writes are permitted, with one distinction that matters.** The grant
  permits `INSERT`, `UPDATE`, `DELETE` and DDL on every database. This section
  used to add "this app reads and must keep reading"; **that blanket rule was
  lifted on 2026-11-26** and is recorded here rather than erased, because the
  reasoning behind it still applies to part of the surface.

  What is still true: **writing into a table GLPI itself manages corrupts the
  install in ways the UI will not show.** GLPI maintains its own history,
  entity scoping and derived state for its own tables, and an `UPDATE` behind
  it skips all three. Where a write must land in GLPI's own data, go through
  the API or GLPI's own code, not a query.

  What the lifted rule was blocking: tables GLPI does **not** manage — a
  plugin's own `glpi_plugin_*` tables, for instance, where there is no history,
  no derived state and no entity scoping to bypass. Writing those directly is
  not the hazard above, and is now allowed. Gate each such write behind its own
  `GLPI_INV_*_WRITES` flag like every other write path here.

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

## What this app writes, and what it does not

(The next section's title claimed the app writes nothing. That stopped being
true on 2026-11-15, twice over: `POST /api/mariadb/innodb/{name}` sets InnoDB
settings over SQL behind `GLPI_INV_INNODB_WRITES`, and `POST /api/db/restart`
runs `scripts/db-restart.sh` behind `GLPI_INV_DB_RESTART`. Both are off by
default. The findings below are what the deleted engine page established and
are unaffected.)

A fourth write landed on 2026-11-21, and it is the only one that is **not**
behind a flag: the backend creates `glpi_inv` - its own schema - at startup,
with a `status_samples` table and a `sample_status` EVENT that snapshots 16
`GLOBAL_STATUS` counters a minute and prunes past 14 days. It also sets
`event_scheduler = ON`, because an event that exists and never fires leaves
an empty table and reports nothing.

**Why no flag, when the other three have one.** Those gate changes to
something someone else owns - the server's running configuration, its startup
file, its lifecycle - where the blast radius is the whole install. This
creates one schema belonging to this app and writes rows to it. A flag would
also defeat it: off by default leaves every fresh install with no sampler,
which is the thing it exists to fix.

**What has not moved**: nothing here touches `glpi`. A history table inside
GLPI's schema would have broken that rule for a dashboard's convenience, and
`glpi::sampler` has a test asserting the generated SQL never qualifies
anything with `glpi.`.

Three things worth not undoing:

- **It never fails startup.** The account may not hold CREATE, or this may be
  a server the app only reads. Six of the seven pages do not care, so a
  failure is logged and the history panel renders its "nothing is sampling"
  state, which is already a state it has and a truthful one.
- **The event is generated from `SAMPLED_COUNTERS`**, not written out beside
  it. The counter list existed in two places for a day - an `IN` list on the
  server and the Rust const - and they drift silently. `StatusHistory::missing`
  was built to report that drift and is kept for the case it still catches: an
  event edited by hand on the server.
- **`IF NOT EXISTS` on the schema and table, `CREATE OR REPLACE` on the
  event.** Existing samples are never dropped; the event holds no state, only
  a schedule, so replacing it costs nothing and lets a changed counter list
  take effect on the next start without a migration.

### Session expiry: two php.ini keys, and the one that silently loses

A fifth write landed on 2026-11-26, behind `GLPI_INV_PHP_WRITES` and pointed
at `GLPI_INV_PHP_TUNING`. Same shape as the nginx tuning file: one Nix file
this app writes, imported by the system config, applied by `nixos-rebuild
switch` and never by the app - php.ini is a read-only store path and PHP has
no runtime settings interface, so there is no apply-it-now at any price.
`glpi::logout` owns it and the Logout tile on the home page is its only
reader.

Three things worth not undoing:

- **A key glpi.nix's `php.buildEnv { extraConfig }` already sets cannot be
  written from here, and the failure would be silent.** The phpfpm module
  builds the pool's `-c` ini as `cat ${phpPackage}/etc/php.ini
  $phpOptionsPath`, then PHP scans `${phpPackage}/lib`, where `buildEnv` puts
  its own php.ini - and a scanned file is loaded last and wins. Measured, by
  appending to a copy of this pool's real `-c` ini and reading it back
  through the pool's own php binary:

      session.gc_maxlifetime = 28800   ->  28800   (absent from extraConfig)
      memory_limit = 333M              ->  256M    (extraConfig sets it)
      session.cookie_httponly = 0      ->  On      (extraConfig sets it)

  So `Owner::Package` refuses those keys rather than writing a file that
  rebuilds clean and changes nothing. The corollary: do **not** tidy
  `session.gc_maxlifetime` into that `extraConfig`, which would turn the
  control into one of them.

- **`login_remember_time` is read and never written.** It is a row in
  `glpi_configs`, which GLPI manages - history, cache in `$CFG_GLPI` - so
  the rule above sends it through GLPI's own code, and GLPI's API has no
  config write at any version (`getGlpiConfig` reads; nothing writes). The
  row carries a link into GLPI's General setup instead of a field. It is
  read over **v1** `getGlpiConfig`, not SQL: `getSafeConfig()` strips only
  the eight `Config::$undisclosedFields` secrets and this is not one.

- **`session.gc_maxlifetime` has a 60-second floor.** PHP reads 0 as
  "eligible for collection immediately" and `CronTask::cronSession` reads
  the same 0 as "fall back to a week", so at 0 the two halves of one setting
  disagree about what was asked for.

GLPI has no session-duration setting of its own, which is the whole reason
the tile exists: the figure everyone calls "the GLPI logout time" is PHP's,
and the figure GLPI does own (`login_remember_time`) is a cookie lifetime
and not a timeout.

A third write landed on 2026-11-20: `POST /api/mariadb/variables/{name}`, the
general case of the InnoDB one, behind its own `GLPI_INV_VARIABLE_WRITES`.
**It is not a widening of the InnoDB flag and must not be folded into it.**
That one admits 106 settings of one subsystem; this one admits every dynamic
variable the server has — 484 of this server's 693, the other 209 being 196
the server refuses at runtime and 13 that are SESSION ONLY and have no global
value at all. `settable` on each row is what says which, and it is **not**
`!read_only`: the live tier is cut on `READ_ONLY` alone, so those 13 sit in it
and cannot be written.

Three things about it worth not undoing:

- **The validation was never InnoDB-specific.** `literal_for` builds its
  literal from `VARIABLE_TYPE`, `ENUM_VALUE_LIST` and the numeric bounds that
  `SYSTEM_VARIABLES` publishes for every row, and the name used in the SQL is
  the server's own spelling of a row it just reported. Only the `LIKE
  'INNODB%'` filter made the old write InnoDB-shaped. Keep the literal built
  from a parsed number or a matched enum entry; never from submitted text.
- **`glpi::hazard` is nine names that are valid to set and break something
  else** — the connection limit that locks everyone out, the buffer pool this
  host has not the memory for, the character set GLPI's tables are created
  with. They are answered `409` with the reason and set only on a second
  request carrying `confirm`. It is deliberately **not** a deny list; the deny
  list is `db-restart.sh`'s `DENIED`, and the distinction is recoverability —
  nothing in `hazard` leaves a server that cannot start, and everything in it
  is recoverable only *from* a running one.
- **`409` rather than `400` for the confirm case.** "Come back and mean it"
  and "the server will not take that value" look identical at 400 and need
  opposite things from the reader.

### Making a change outlive a restart

**Setting a variable keeps it.** `POST /api/mariadb/variables/{name}` writes
the running value *and* the startup-file line, in that order. The two were
separate endpoints at first and the split was wrong in practice: a value set
and not kept reverts at the next restart with nothing on the row saying it
will, so the control that appeared to work was the one that silently did not
last. Un-keeping is now the deliberate act, which is the right way round —
it is the reversible one.

A failure to keep does **not** fail the request. The server has already taken
the value, and answering with an error would describe a write that happened as
one that did not. It comes back as `kept: false` and the row says so. (The
InnoDB panel's `set_innodb` is deliberately unchanged — it has its own flag and
its own panel, and widening its behaviour is not what that flag was granted
for.)

`POST /api/mariadb/variables/{name}/persist` remains, for the two things
setting cannot express: keeping a value you did not change, and dropping a
line without touching the running server. A request with no body drops it. It
goes through the same `variable_literal` gate as the write, and that is not
tidiness: a line the server refuses at startup **silently drops every line
below it** (measured — the server logs the error, startup continues, the file
stops executing there), so a value that could not be set must not be
persistable either.

`glpi::init_file` — named `innodb_init` until 2026-11-20 — owns the file for
both the InnoDB panel and the variables pages. There is one file and one
format; only what `SET GLOBAL` can set may go in it.

**This was a one-time root step, and it has been done** - on this machine the
directory, the file, the `my.cnf` entry and `GLPI_INV_INIT_FILE` all exist, so
`persist_path` is set and the pages say a change outlives a restart. The setup
is kept here because it is what a new machine needs, and because the three
traps below are what made it take two attempts:

```nix
services.mysql.settings.mysqld.init_file = "/var/lib/glpi-inv-init/persist.sql";
systemd.tmpfiles.rules = [
  "d /var/lib/glpi-inv-init 0750 pde mysql -"
  "f /var/lib/glpi-inv-init/persist.sql 0640 pde mysql -"
];
```

then `GLPI_INV_INIT_FILE` (renamed from `GLPI_INV_INNODB_INIT_FILE`) pointing
at the same path. Three things that bite:

- **It cannot live under `/home`.** The mysql unit runs `ProtectHome=true`, so
  the server cannot see it there.
- **The file must exist before MariaDB is pointed at it.** An init-file mysqld
  cannot open is not a setting that quietly fails: it logs `Can't
  create/write to file … (Errcode: 2)`, then `Aborting`, and exits 1. Hence
  the `f` tmpfiles rule and not just the `d`.
- **A drop-in cannot be used instead.** mysqld runs
  `--defaults-file=/etc/my.cnf`, which replaces the whole search path, so
  `--defaults-extra-file` and `/etc/my.cnf.d` are both dead ends here. The
  only routes are this file and `services.mysql.settings`.

### Restarting the database: the OS decides, and the script asks

`scripts/db-restart.sh` is the only thing that knows how. It detects the OS
first because the answer differs completely - on NixOS the plain restart is a
polkit prompt away while changing a startup-only setting cannot be done from
the app at all (`/etc/my.cnf` is a read-only store path generated from
`services.mysql`), on Debian/RHEL both work, on Windows the file is writable
but there is no polkit to ask with.

On NixOS that is a *write* it cannot do, not a restart it cannot do, and the
distinction matters: `mysql.service` carries `X-Restart-Triggers` naming
my.cnf's store path, so a changed setting gives a new path, a changed unit and
a restart performed by `nixos-rebuild switch` itself. The rebuild IS the
restart - nobody should click the restart link afterwards - and a restart
without a rebuild can never apply one of the 31. The script says so; it said
the opposite by omission until 2026-11-15.

`backend/src/db_restart.rs` only finds the script and returns what it said,
verbatim, with its exit code: 0 done, 2 refused, 3 not possible here, 4 tried
and rolled back.

Three things worth not undoing:

- **Neither link restarts on the first click.** The first runs with `DRY_RUN`
  and prints the commands; a second, separate click runs them. Previews are
  allowed even when the flag is off, so a disabled control can still explain
  itself.
- **`innodb_page_size`, `innodb_data_file_path` and `innodb_undo_tablespaces`
  are denied by name**, not by validation. They are valid values that leave
  the server unable to start against its existing datadir, and a config file
  keeps it that way across every later restart. No validation catches them
  because the value is not what is wrong -
  `mariadbd --help --verbose --innodb-page-size=64K` passes clean.
- **`mariadbd` has no `--validate-config`, and passing it starts a server.**
  That is a MySQL 8 flag. The unknown option is not reported until after the
  storage engines initialise, so the command writes a datadir and then
  aborts - measured at 145MB into `--datadir`, which was `/tmp`, and
  `DRY_RUN` did it too. Its own `[ERROR] unknown option` line then matched
  the script's error grep, so every valid setting came back as *rejected by
  the server*. Fixed on 2026-11-18; `validate_settings` in
  `scripts/db-restart.sh` uses `--help --verbose`, which parses the same
  options through the same code, writes nothing, and reports on stderr.
  `~/docs/gl-win-install.md` asserted the flag existed and gave a
  verification for it; the test used `innodb_page_size=7777`, which InnoDB
  refuses early enough that the real error looks like validation working.
- **106 of this server's 137 InnoDB settings never need any of this.** They
  are dynamic (`SYSTEM_VARIABLES.READ_ONLY = 'NO'`), so SQL changes them and
  the init-file keeps them. Only 31 need a file and a restart. A restart-
  shaped control is the wrong instrument for the other 106.

Full reasoning, per OS, with the measurements: `~/docs/gl-win-install.md`.

## There is no engine page

Both existed for part of 2026-11-14 and were removed the same day: a `maria`
sub-tab with a strip of eight engine tabs, and a `POST /api/mariadb/engine`
behind a `GLPI_INV_ENGINE_WRITES` flag that ran `ALTER TABLE ... ENGINE=`.
Do not rebuild either without reading this. Recoverable at `36d05b0` (the
tabs) and in this file's history (the write).

What the exercise established, all measured on this box rather than reasoned
about:

- **GLPI wraps each inventory import in one transaction** —
  `src/Glpi/Inventory/Inventory.php:328`, `commit()` at 460, `rollback()` in
  the `catch`. One import touches the asset, its software, its ports and the
  log queue. On a non-transactional engine `beginTransaction()` is accepted
  and ignored, so a failed import leaves a half-written asset that *looks*
  current — recent `last_contact`, rows present, nothing marking which are
  leftovers. That is worse than the failure this app exists to surface, and
  it is why moving a GLPI table off InnoDB is never the right answer: the
  only engines MariaDB will accept are the ones GLPI cannot afford.
- **GLPI 12.0.1 declares no foreign keys** — zero `FOREIGN KEY` in
  `install/mysql/glpi-12.0.0-empty.sql`, zero rows in
  `information_schema.REFERENTIAL_CONSTRAINTS`. Referential integrity is in
  PHP. Worth re-checking on a GLPI upgrade.
- **A declared foreign key blocks the conversion outright** on MariaDB
  11.4.12 — errno 1217, from the child side, the referenced side, and with
  `foreign_key_checks=0`. Silently dropping the key is older MySQL
  behaviour, not this server's.
- **Of the eight engines compiled in, three accept a GLPI table** (InnoDB,
  Aria, MyISAM), four refuse structurally (MEMORY 1163 no BLOB/TEXT, CSV 1164
  no AUTO_INCREMENT, SEQUENCE 1005/131, PERFORMANCE_SCHEMA 1031) — and
  **MRG_MyISAM accepts the ALTER and returns an empty table**, no error and
  no warning, because a MERGE over no underlying tables is a legitimately
  empty view. It is the only silent data loss of the eight.

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
