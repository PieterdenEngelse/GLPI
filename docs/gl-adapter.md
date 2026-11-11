# The adapter layer — how glpi-inv reaches GLPI, and what each route costs

`glpi-inv` needs things GLPI's current API does not expose. It currently uses
**four** different ways in, and a **fifth** was proved possible on 2026-11-05
and is not used. This file is what is known about each: the mechanics, the
failure modes, and who owns the instability.

Written because the knowledge was scattered - the client's own doc comments,
`gl-rights.md`, `gl-db-api.md`, `gl-upgrade.md`, and a throwaway plugin that
no longer exists. Everything below was measured against GLPI **12.0.1** unless
it says otherwise.

- **The thing to know first:** all four adapters in use have a failure mode
  that is silent, and they are silent in *different directions*
- **The newest and worst:** an RSQL filter that fails **open**
- **The newest** is the only one that steps around an access control GLPI
  added deliberately, and the only one whose silent failure *under*-reports
- **The unused fifth** removes the quiet SQL coupling and replaces it with a
  louder, larger one

---

## 1. v2.3 High-Level API — the default

`backend/src/glpi/mod.rs`. Everything asset-level: agents, computers,
entities, users, session.

### What is settled

- **One line builds the version.** `get()` delegates to
  `get_at(HL_DEFAULT, path)`; `HL_DEFAULT` is `v2.3`. Before this was split
  out, v2.3 was the only version the client could reach at all.
- **v2.3 is the default deliberately.** v3.0 is not a superset - `Router.php`
  serves 1, 2.3 and 3.0, and **42 routes are removed at 3.0** (the
  `SoftwareLicense` compatibility shapes and the knowledge-base `/Category`
  endpoints). Every schema this app reads is verified against v2.3 and none
  against v3.0.
- **The pagination contract** is `start`/`limit`, a `Content-Range:
  start-end/total` header, and 206 meaning more remain. The block in
  `ResourceAccessor.php` that sets the status and the header is
  **byte-identical between 11.0.12 and 12.0.1**, and the three measurements
  reproduce on both:

  ```
  Assets/Software                       ->  206  Content-Range: 0-99/1330
  Assets/Software?limit=500             ->  206  Content-Range: 0-499/1330
  Assets/Software?start=1300&limit=100  ->  200  Content-Range: 1300-1329/1330
  ```

- **Authentication is the OAuth `password` grant**, not
  `client_credentials`: `Router.php:717` requires a logged-in user and a
  client-only token has none. The client retries once on 401, because the
  token rides on a server-side session that can end while the JWT still looks
  valid.
- **It describes itself.** `/api.php/v2.3/doc.json` is an OpenAPI 3.0.0
  document, `security_level: SECURITY_NONE`, ~4.6 MB, generated per request
  and **not** filtered by the caller's rights. That asymmetry is what makes
  the field-gap check possible.

### Quiet failure A — a field suppressed by rights

GLPI's HL schemas carry `x-rights-conditions`, and they **degrade rather than
refuse**, in two shapes:

- **object-level**, filtering rows: `User` joins `glpi_profiles_users` and
  adds a `WHERE` on the session's entities when the caller cannot view all
  entities. Present in both versions.
- **field-level**, dropping columns: a field the caller may not read is simply
  absent from an otherwise successful 200.

The field-level shape is **new in 12.0.1**. In `AdministrationController`,
`x-rights-conditions` goes from **4 to 16**, with `User::READAUTHENT` and
`Notification::canView()` appearing for the first time - gating
`User.date_sync`, `Group.mfa_enforced` and nine `Entity` notification-email
fields.

An absent field is indistinguishable from a null one, because every field in
this app is `Option` by design. That design is right, and it is exactly what
hides this.

**Detected, not merely known about:** `glpi/schema_check.rs` compares the
declared schema against what arrived. Measured across both versions with the
same binary:

| schema | 11.0.12 | 12.0.1 |
|---|---|---|
| `User` | ok 29/29 | **attention 30/29 - `date_sync`** |
| Agent / Computer / Entity / Session | ok | ok |

Write-only properties (`User.password`, `password2`) are excluded - only ten
such properties exist across all 330 schemas, so the exclusion is precise
rather than a heuristic.

### Quiet failure B — an RSQL filter that is silently ignored

**The worst of the three, because it fails open.** `Search.php:443`:

```php
$other_invalid_filters = array_filter($filter_result->getInvalidFilters(),
    static fn($error) => $error !== Error::UNKNOWN_PROPERTY);

// Fail the request if any of the filters are invalid
if ($schema_resolver === null && $other_invalid_filters !== []) {
    throw new RSQLException(...);
}
```

`UNKNOWN_PROPERTY` is deliberately lifted out of the set that throws. An
unknown **operator** and a mapped property raise and name the term; an unknown
**field** does not. Upstream's own comment overstates the code beneath it.

Measured, counting rows on `Assets/Computer`:

| filter | rows | |
|---|---|---|
| *(none)* | 1 | |
| `name=="nixos"` | 1 | applied |
| `name=="no-such-machine"` | 0 | applied |
| `no_such_field=="x"` | **1** | **ignored** |
| `is_deletedX==0` | **1** | **ignored** |

Compare the directions:

- a rights-suppressed field → a blank cell
- a renamed SQL column → a blank cell
- **an ignored filter → more data than was asked for, with a 200**

**The rule this leaves:** a server-side filter in this app is an
*optimisation*, never a *guarantee*. Anything excluded for correctness is
excluded again locally. `api::computers` does exactly that for `is_deleted`
and logs when GLPI evidently ignored the term.

`backend/src/glpi/rsql.rs` builds filters safely - values quoted and escaped,
because RSQL ends an unquoted value at the first unescaped `,`, `;` or `)`,
so a machine named `foo;bar` would otherwise become two terms. Sixteen
operators exist in 11.0.12 and eighteen in 12.0.1; `=istruthy=` and
`=isnottruthy=` are the only additions.

---

## 2. v1 legacy API — for what HL does not model

`backend/src/glpi/legacy.rs` and `glpi/inventory.rs`, via `/apirest.php`.

Five calls with **no v2.3 equivalent at any version**: `ProfileRight`,
`Profile`, `Profile_User`, `Entity`, `getFullSession`. Plus the three
fleet-wide inventory collections, which v2.3 serves only beneath a single
asset id.

### What the upgrade settled

**v1 survives 12.0.1.** This was the feared blocker and it is not one -
verified against a running instance, not only the source: all five calls
answer with data (`ProfileRight` 192 KB). `API.php`, `APIRest.php`, the
`Deprecated/` directory and `apirest.md` all ship in the release.

**v3 does not replace it.** Measured on a running 12.0.1:

| v3.0 path | result |
|---|---|
| `Administration/Profile_User` | **404** |
| `Inventory/RefusedEquipment` | **404** |
| HL `Profile` schema | the same eight properties as 11.0.12 - no rights, no `interface` |

### What it costs

Stability, and a second credential. GLPI describes v1 as "closely tied to the
GLPI source code … no promise of stability between versions". It rejects the
OAuth bearer outright and needs Basic auth, which is why
`enable_api_login_credentials` is one of the four config keys this app
depends on.

What it does **not** cost is permissions: every v1 call is still made as the
service account and still filtered by its profile and entity scope. That is
the precise difference from adapter 3.

---

## 3. Direct MariaDB — one grant, two tables

`backend/src/glpi/refused.rs`. `SELECT` on `glpi_refusedequipments` and
`glpi_logs`.

Exists because refused submissions are exposed by **neither** API, and they
are the failure this dashboard most wants to show: the agent exits 0, GLPI
throws the submission away, no asset appears, and nothing in the agent's own
log says so.

### What it costs

- **A renamed column fails silently.** Every column is treated as optional
  and renders blank, so the failure mode on screen is a gap rather than an
  error. This is the coupling `gl-upgrade.md` section 6 calls the only quiet
  one - it is not, but it is still quiet.
- **SQL has no notion of entities.** The account could read every row
  regardless of scope, so rows are restricted afterwards to the entities the
  API says this account may see. The scoping is reconstructed, not inherited.
- **`glpi_logs` is granted and not yet read, and that is provisioning rather
  than excess.** GLPI exposes **no field-level change history under Inventory
  at any API version**, so "what changed between runs" is unreachable by any
  other route. A serial or MAC that silently changed between inventories - a
  hardware swap, a reused hostname, a duplicated device - is the natural
  fourth quiet failure for an app whose whole subject is quiet failure, next
  to "stopped reporting", "never landed" and "no agent". The grant was taken
  deliberately against that gap, and the gap still exists.
  **Do not revoke it as a tidy-up.** Nothing in the code reads it, so an
  audit by present usage reads it as cruft; that reading is wrong, and this
  paragraph exists because an earlier draft of this file made exactly that
  mistake and recommended revoking.
- No password is stored: the account authenticates over the unix socket.

---

## 4. Log files on disk — enumerate to list, allowlist to read

`backend/src/logs.rs`. The tail of `access-errors.log` and `php-errors.log`
out of GLPI's own log directory, for the Logs page.

Exists because an access refusal leaves its only trace there - and, as it
turned out, because the migration logs answer provenance questions no API or
grant reaches; see *What this directory answers* below. Note the gap this
does **not** duplicate: `RefusedEquipment` (adapter 3) records that a
submission was thrown away by an import *rule*; an inventory refused for
*access* - endpoint disabled, right missing - never reaches a rule and so
never reaches that table either.

### Which two, and why `api.log` was the wrong first answer

Both chosen by finding the code that writes them, not by reading the
filenames. GLPI checks ten log files for write access
(`LogsWriteAccess::LOG_FILES`) and has only **two** Monolog handlers:

| file | written by | contents |
|---|---|---|
| `access-errors.log` | `AccessLogHandler` | **only** refusals - `isHandling()` returns false for anything that is not an `AccessDeniedHttpException` |
| `php-errors.log` | `ErrorLogHandler` | errors and uncaught exceptions; where a 500 from either API lands, and in GLPI 12 where SQL failures surface - there is no `sql-errors.log` |

The first of those is the most on-subject log GLPI keeps: a disabled
inventory endpoint throws exactly that exception
(`InventoryController.php`, see `gl-net.md`), so an agent refused for access
is a line in that file and nothing else anywhere.

**`api.log` was picked first and was wrong on three counts**, all verified:

- written from exactly **one** place, `Glpi/Api/API.php:2406`, which is the
  **legacy v1** API - the HL API never writes it;
- only when the calling API client record has `dolog_method = DOLOG_LOGS`,
  and the column is `tinyint NOT NULL DEFAULT '0'` = `DOLOG_DISABLED`;
- and `logEndpointUsage()` records **successful** endpoint usage, not
  refusals.

So the tab it would have filled would have stayed empty on this install
indefinitely, for reasons nothing in the UI could have explained. Recorded
here because the mistake was to reason from a filename - the same error this
file exists to stop being repeated from memory.

### Why not through GLPI — all read in the 12.0.1 source

- **Neither API exposes logs at all.** `system_logs`, `LogViewer` and
  `LogParser` have **zero** occurrences under `src/Glpi/Api/`.
- **The ajax endpoint behind GLPI's own viewer returns HTML.**
  `ajax/logviewer.php` takes `POST action=refresh_log_file` with a `filepath`
  and calls `showLogFile(..., true)` - markup, not JSON. It also calls
  `Session::checkRight(LogViewer::$rightname, READ)`, so using it would mean
  granting `system_logs` READ to this app's service account: widening a
  stolen token from "agents and computers" to every log on the server.
- **The page route additionally demands re-authentication.** `front/logs.php`
  calls `LogViewer::checkReAuthenticationOrRedirect()` *before* the right
  check. GLPI asks for a fresh password for log access on purpose.

### The mechanism

`GLPI_LOG_DIR` is `{GLPI_VAR_DIR}/_log` (`SystemConfigurator.php:109`) and
`glpi.nix` points `GLPI_VAR_DIR` at `/var/lib/glpi/files`, so
`/var/lib/glpi/files/_log`. That directory is `0750 glpi:glpi` and this
backend runs as an ordinary user, so access comes from a POSIX ACL:
`services.glpi.logReaders = [ "pde" ]` emits traversal-only (`x`, **not** `r`)
on the two parents, read on `_log`, and a **default** ACL so logs GLPI creates
later inherit it.

Traversal-only is the design, not tidiness. Adding the user to the `glpi`
*group* would also expose `config/config_db.php`, which holds the database
credentials **and** the encryption key.

**Listing and reading are separate privileges**, and the split is the design:

- **Listing is enumerated, never hardcoded.** `GET /api/logs` does
  `read_dir` and keeps regular files ending `.log`, sorted by name. This is
  what GLPI does too - `LogParser` scandirs `GLPI_LOG_DIR` against
  `/^(.+)\.log$/` - and an earlier version of this module got it wrong by
  listing eight names read out of the GLPI source. The directory holds
  **fourteen**, and three of them could not have been found that way at all:
  `migration_to_12.0.0.log`, `migration_to_12.0.1.log` and
  `warning_during_migration_to_12.0.0.log`, written with computed channel
  names during the 11.0.12 -> 12.0.1 upgrade. A source-derived list of logs
  is a lower bound, never the answer.
- **Reading is an allowlist of two names, not a path parameter.** A sanitised
  path that reaches the filesystem is how a diagnostic view becomes an
  arbitrary file read. `logs::ALLOWED` is checked before any path is built; a
  test asserts `../config/config_db.php` is refused. Each listed entry
  carries a `readable` flag so the UI keeps no copy of that list.
- **Bounded in bytes, not lines.** One log line can be an entire stack trace,
  so a line budget is not a memory budget. A 256 KB window, the last 300
  lines within it, and the partial first line dropped.

### What it costs

- **The only adapter that steps around an access control GLPI added
  deliberately.** Adapter 3 bypasses entity *scoping* - which GLPI never
  applied to those two tables anyway, and which `refused.rs` rebuilds by
  hand. This one bypasses `system_logs` and a re-authentication prompt, a
  barrier GLPI put in front of its own UI. And because the grant lives in the
  filesystem rather than in GLPI, **GLPI cannot revoke it**: nothing in its
  profile screens will ever show that this app reads those files.
- **A log format is not a contract.** A renamed column at least has a schema
  to diff across releases. What a Monolog handler writes, and whether a given
  file gets written at all, is an implementation detail with nothing attached
  to compare - as `api.log` above demonstrates at this app's expense.
- **The silent failure runs in a new direction: it under-reports.** The other
  three fail by returning less than was asked for while reporting success - a
  suppressed field, a blank column - or, worse, more than was asked for. This
  one fails by looking in the wrong place and calling it quiet. The reader
  distinguishes `Ok` / `Absent` / `Empty` / `Unreadable`, and `Absent` is
  rendered as "GLPI creates each log on first write, so this has had nothing
  to say". That reading is correct only if the path is right.
  **Enumeration made this louder than it was.** When the tab strip was a
  hardcoded list, a moved `GLPI_VAR_DIR` showed fifteen tabs all reporting
  `Absent` - indistinguishable from a quiet server. Now a wrong directory
  shows **zero** tabs, which is visibly wrong rather than quietly wrong. The
  failure is not eliminated: an empty-but-correct directory and a wrong path
  still render the same, and only the first is a real answer.
- **Nothing cross-checks the two paths.** GLPI's log directory is configured
  in `glpi.nix`; this app's copy of it is configured in
  `~/.config/glpi-inv/env` as `GLPI_INV_LOG_DIR`. They are set independently,
  in different files, and **no API exposes `GLPI_LOG_DIR`** - so there is no
  route by which the app could verify it is reading GLPI's logs rather than
  an empty directory of the same name. This is the one cost here with no
  mitigation in place.

### What is loud

Permission is loud and specific, which is the failure most likely to happen:

```
GET /api/logs/access-errors.log
{"configured":true,"state":"unreadable",
 "detail":"Permission denied (os error 13)", ...}
```

`configured: false` is a separate state again, for an unset
`GLPI_INV_LOG_DIR` - a different fix from a directory the kernel refuses, and
the page says which.

**Deployed state as of 2026-11-06:** live, ACL granted, enumeration
returning all fourteen files with byte counts. `php-errors.log` reads
`state: "ok"` with GLPI `WARNING` and `CRITICAL` lines from 2026-10-30.

The log files themselves are `0644`, so the directory was the only barrier -
traversal plus read on `_log` was sufficient, and the **default** ACL matters
only for logs created from here on.

Two files worth noting from the first real listing:

| file | what it settled |
|---|---|
| `api.log` | **0 bytes**, exactly as `dolog_method DEFAULT '0'` predicted. The swap away from it was not a judgement call, it was the only option |
| `cron.log` | **509 KB**. The byte-bounded window earns its keep here; reading this one whole to show its last screen would have been the obvious wrong implementation |

### What `access-errors.log` actually contains

Recorded because section 4's whole justification rested on inference until
this was read. The file holds **three** entries, all from
`2026-10-31 07:27:19`:

```
[2026-10-31 07:27:19] User ID: `Anonymous` tried to execute an invalid
request on `/Inventory`.
  Additional information: Inventory is disabled
  Backtrace :
  ./src/Glpi/Controller/InventoryController.php:67
```

The chain is confirmed end to end, on data rather than on reading:

- The message is `AccessDeniedHttpException("Inventory is disabled")`,
  the literal string in `InventoryController::index()`, thrown when
  `$conf->enabled_inventory != 1`.
- Both `/Inventory` **and** `/front/inventory.php` appear, because that one
  method carries two `#[Route]` attributes - which is also why `gl-net.md`
  reports both endpoints 403ing before `enabled_inventory` was set.
- The timestamp is during the original build, before
  `config:set --context=inventory enabled_inventory 1` was run. This file is
  the trap in `gl-client-serv.md` §2.7, recorded by GLPI as it happened.

**What it does NOT show:** no agent appears in it. All three entries are
`Anonymous` from a manual probe in the same second. So the *mechanism* is
proved - a disabled endpoint lands here - while "an agent refused for access
appears in this file" remains inference, just a much shorter one.

Nothing in the file is sensitive: no addresses, no usernames beyond
`Anonymous`, no credential-shaped strings. Checked before quoting it here.

### What this directory answers that nothing else does

Two questions have now been settled from these files that neither API could
touch. Recorded because section 4 justified itself on refusals alone, and
refusals turn out to be the smaller half.

**1. A refusal, which is what the adapter was built for.** Covered above:
`access-errors.log` holds the `AccessDeniedHttpException` text and nothing
else, so a request GLPI turned down is a line there and nowhere else.

**2. Provenance — what an upgrade actually did.** On 2026-11-06 the question
was whether the two native Forms on this install had been created by the
11.0.12 -> 12.0.1 upgrade or by hand. Nothing could answer it:

- Neither API exposes schema migrations.
- `glpi_logs` (adapter 3) is **item** history, not schema history - a
  distinction worth keeping straight, because the grant's justification in
  section 3 reads as "change history" and this is a change it does not cover.
- `glpi_forms_forms` is not in the database grant, so the rows could not even
  be counted.

`migration_to_12.0.0.log` answered it in one line out of 302 - `Change of the
database layout - glpi_forms_categories`, a layout change to the *categories*
table and no insert anywhere - corroborated by the 11->12 form migration
containing zero `INSERT` or `->add()` calls. The upgrade did not create them.

That class of question - *what changed, when, by whose hand* - is the one this
directory is uniquely good for, and it is adjacent to the quiet failure the
`glpi_logs` grant was taken against. Worth knowing before that grant is ever
revisited.

### The honest gap this opened

**The file that proved most useful is one this app cannot read.**
`migration_to_12.0.0.log` is listed - name and 12.4 KB - and is not in
`logs::ALLOWED`. The question above was answered with a shell, not through
the adapter.

So the two-name allowlist is now a decision with evidence against it as well
as for it. The case for keeping it: the migration logs are read once per
upgrade, by someone who has a shell anyway, and GLPI's own viewer shows them.
The case for widening it: the app already has the directory open, the
listing already shows these files exist, and a reader who can see
`migration_to_12.0.1.log` in the tab strip and cannot open it is being shown
a locked door.

**Decided on 2026-11-08: keep the two.** The deciding point is the simplest
one and it was missed above - **GLPI's own viewer shows all of them**, and
whoever runs this dashboard has the `system_logs` right to use it. So the
twelve are not unreachable, only unreachable *from here*, and each of their
tabs already carries a button into the screen that does show them. Widening
would duplicate a complete feature with a worse one: no search, no filtering,
a 256 KB window, and a second access path to maintain.

That also reframes "the locked door" argument. It is not a locked door, it is
a signpost - which is the right thing for this app to be about a log that is
not its subject.

Revisit only if GLPI's viewer stops being usable for this, which would mean
losing the `system_logs` right or the re-authentication becoming impractical.
Neither is in prospect.

---

## 5. A GLPI plugin — proved, unused

The fifth way in, and the only one that could remove adapter 3 entirely.
Proved on the throwaway 12.0.1 instance with `refprobe`, a one-route
read-only plugin, then torn down.

### The mechanism

`Router.php:274-286`, after GLPI registers its own eleven controllers:

```php
if (isset($PLUGIN_HOOKS[Hooks::API_CONTROLLERS])) {
    foreach ($PLUGIN_HOOKS[Hooks::API_CONTROLLERS] as $controllers) {
        foreach ($controllers as $controller) {
            if (is_subclass_of($controller, AbstractController::class, true)) {
                self::$instance->registerController(new $controller());
            }
        }
    }
}
```

Two hooks: `api_controllers` and `api_middleware` (`Plugin/Hooks.php:1123`,
`:1132`).

### What was observed

```
GET /api.php/v2.3/Refprobe/RefusedEquipment   ->  200  []
Content-Range                                 ->  0--1/0
doc.json schema count                         ->  331   (was 330)
  components.schemas.RefusedEquipment         ->  present, all 12 properties
  paths./Refprobe/RefusedEquipment            ->  present
```

The empty array is correct - this install has refused nothing. What is
established is that a plugin-declared schema **reaches the OpenAPI document
and the search machinery** and carries a `Content-Range`. It inherits the
pagination walk, RSQL, `x-rights-conditions` and GraphQL for free, because
`getRawKnownSchemas()` is the same entry point the built-in controllers use.

`RefusedEquipment` is an ordinary `CommonDBTM` with
`$rightname = 'refusedequipment'`, and **this app's account already holds that
right at READ** - so exposing it needs no privilege change, and the result
would be entity-scoped by GLPI instead of re-scoped afterwards.

### The mechanics, all of which bit at least once

- **Packaging is free.** `SystemConfigurator.php:94` makes the production
  default `['{GLPI_MARKETPLACE_DIR}', <root>/plugins]`, and `glpi.nix:149`
  already points `GLPI_MARKETPLACE_DIR` at `/var/lib/glpi/marketplace` -
  writable, outside the store, and **first** in the search order. No
  derivation patch, no new environment variable, no `glpi.nix` change.
- **The class name is load-bearing.** The autoloader matches
  `/^Plugin([A-Z][a-z0-9]+)([A-Z]\\w+)$/` (`misc-functions.php:78`), splitting
  `PluginRefprobeController` into plugin `Refprobe` + class `Controller`, then
  loading `<plugin>/src/PluginRefprobeController.php`. Rename either half and
  the class silently never resolves.
- **`plugin_<key>_install` is mandatory**, even for a plugin owning nothing.
  `Plugin.php:1245` reports "function is missing" rather than treating absence
  as nothing-to-do. Same for `_uninstall`.
- **`plugin:list` shows registered plugins, not discovered ones.** It was
  empty until `plugin:install` ran - that is the step that scans directories.

### The cost, and it is the decisive one

**A malformed plugin route takes down the entire High-Level API.**
`RoutePath::getRouteVersion()` is
`getAttributes(RouteVersion::class)[0]->newInstance()` with no guard, and
`matchesAPIVersion()` calls it from an `array_filter()` over **every**
registered route. A route missing `#[RouteVersion]` throws while GLPI is
merely *enumerating* routes: `/api.php` and every endpoint returned 500, with
nothing in the response naming the plugin. Only
`files/_log/php-errors.log` said why.

So the blast radius of a plugin defect is **total**, not confined to the
feature it adds. Against that: it failed loudly, immediately, on a throwaway.

**Not exercised:** version compatibility. `requirements.glpi = {min: 12.0,
max: 13.0}` was accepted, but no upgrade has run against a plugin to watch
GLPI disable it at the boundary - which is the failure this would have to be
trusted not to cause quietly.

---

## Choosing between them

The honest framing is not "which is technically better" but **whose
instability you prefer to own**.

| | adapter | silent failure | who owns the risk |
|---|---|---|---|
| asset data | v2.3 | suppressed field; ignored filter | upstream |
| rights, authorizations, fleet inventory | v1 | none observed | upstream, with no stability promise |
| refused submissions | SQL | renamed column | you, via the schema |
| access refusals, GLPI errors | log files | `Absent` looks like a quiet server | you, via a path set twice in two files |
| *(possible)* refused submissions | plugin | version-boundary disable | **you, via PHP** |

Today: two APIs plus a grant, and upstream's instability is upstream's
problem. v1 is "closely tied to the source code" but GLPI keeps it working -
it survived a major version intact.

With a plugin: one API, no grant, no second credential, correct entity
scoping - and PHP maintained against `AbstractController`, `Doc\\Schema` and
`ResourceAccessor`, which carry no stability promise either, plus a component
GLPI will disable outside its declared range.

**The narrow case for it** is `RefusedEquipment` alone: it is the only adapter
that costs a database grant, and the only one whose scoping has to be
rebuilt by hand. The two profile gaps are better left on v1, which already
works and survived the upgrade.

---

## The recommendation: keep the grant, do not build the plugin

Four reasons, heaviest first.

### 1. The blast radius is backwards, and it was observed

Today there are three independent failure domains:

| adapter breaks | what degrades |
|---|---|
| SQL | the refused page |
| v1 | the Rights page and the inventory pages |
| v2.3 | the core pages |

The refused view is the **least**-exercised feature on this install - zero
rows - and it is served by the **most**-isolated adapter. A plugin inverts
that: a defect in the least important feature returns 500 on `/api.php` and
every endpoint.

That is not a worry, it is a measurement. One missing `#[RouteVersion]` did
exactly that while building the probe, and nothing in the response named the
plugin responsible.

### 2. The coupling it removes just proved stable at the worst moment

A major version bump is the event most likely to rename a column.
`glpi_refusedequipments` came through 11.0.12 -> 12.0.1 with **all fourteen
columns identical**:

```
id  name  itemtype  entities_id  ip  mac  rules_id  method
serial  uuid  agents_id  autoupdatesystems_id  date_creation  date_mod
```

The silent failure mode in section 3 is real in principle and has now
survived one real test of it.

### 3. It trades a stable coupling for an unstable one

`glpi_refusedequipments` is a database table with a conservative history.
`AbstractController`, `Doc\Schema` and `ResourceAccessor` are internals with
no stability promise, and they visibly churned in that same upgrade - rights
conditions 4 to 16 in one controller, 42 routes removed at v3. The plugin
moves this app's dependency off the quiet stable thing and onto the noisy
unstable one.

### 4. The price is disproportionate

A PHP component, a second toolchain in a Rust project, a deployment artifact
in a Nix-managed system, and an untested version-boundary disable - to remove
a `SELECT` grant on a socket-authenticated account that stores no password
and is scoped to two tables.

### 5. A retained SQL adapter removes most of the plugin's point

This is the sturdiest of the five, and it only became visible after the other
four.

The plugin's entire value is *drop the grant, drop `mysql_async`, drop the
second credential*. That holds only if the SQL adapter goes away **entirely**.
It does not: `glpi_logs` is held for the unbuilt change-history feature (see
section 3), and nothing but SQL can serve it.

So a plugin would remove `glpi_refusedequipments` and leave the grant,
`mysql_async` and `refused.rs`'s infrastructure standing anyway. It stops
being a simplification and becomes **a second API surface added on top of one
you keep**.

### What NOT to do

An earlier draft of this section recommended revoking `glpi_logs` on the
grounds that nothing reads it. That advice was wrong and has been removed.
It judged a resource by present usage on an app that is still being built -
the same error as judging GLPI 12 only against the gaps the app already works
around. The grant is provisioning; see section 3.

Leave the DB page's Schema tab watching `glpi_refusedequipments`, and revisit
the adapter question only if **upstream** exposes `RefusedEquipment` natively,
which would cost nothing here.

### What would change this advice

- **The app grows further SQL needs.** One feature behind a grant is a
  per-feature decision; three would be an architectural one, and the plugin
  would start paying for itself.
- **A genuinely multi-entity GLPI.** The hand-rebuilt entity scoping in
  `refused.rs` is a tidiness problem on a single-entity install and a
  correctness liability on a real tree. That is the argument that would
  actually flip this, and it does not apply here yet.
- **Upstream exposing `RefusedEquipment`.** Then none of this matters and the
  grant goes for free.

---

## Untested

- **Version compatibility of a plugin across a GLPI upgrade.** The single
  thing that would decide the adapter-5 question, and the one thing the
  throwaway could not show.
- **Whether a plugin schema is subject to the same `x-rights-conditions`
  suppression** as built-in ones. It inherits the machinery, so presumably
  yes; not demonstrated, and `refprobe` declared no conditions.
- **The v1 adapter under any GLPI newer than 12.0.1.** It survived one major
  version. That is one data point, not a trend.
- **Whether a refused *agent* appears in `access-errors.log`.** Resolved in
  part: the file exists, and a disabled inventory endpoint demonstrably
  lands in it (see section 4). What is still inference is the agent case
  specifically - every entry in it so far came from a manual probe, not from
  glpi-agent.
- **Rotation.** Nothing establishes whether GLPI or anything else rotates
  these files, or what the byte-window reader returns mid-rotation. A file
  truncated under it would read `Empty`, which the page renders as "nothing
  has been written" - briefly wrong in the under-reporting direction
  described in section 4.
- The RSQL fail-open behaviour was measured on `Assets/Computer` only.
  Nothing suggests it is endpoint-specific - it is in `Search.php`, shared by
  all of them - but it was not re-measured elsewhere.
