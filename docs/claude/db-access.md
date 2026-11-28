# Direct database access, and what the API cannot give

## What v2.3 does not expose

Verified by reading `src/Glpi/Api/HL/Controller/InventoryController.php` in
GLPI 11.0.12:

- **`Agent` IS exposed**, with `deviceid`, `last_contact`, `version`, `tag`,
  `remote_address`, `itemtype`/`items_id`. The app's core data.
- **`RefusedEquipment` is NOT exposed** — zero occurrences. This is the
  failure mode most worth surfacing: the agent exits 0, GLPI rejects the
  submission, the asset silently never appears, and the agent's own log does
  not say so.
- **No field-level change history** (`glpi_logs`) under Inventory. v1 answers
  it per asset via `with_logs=true` — see the last section.

Both live in the database.

## The grant, decided 2026-11-13

`pde@localhost` holds `ALL PRIVILEGES ON *.* ... WITH GRANT OPTION` —
root-equivalent on this server — authenticating through the `unix_socket`
plugin, which is the identity the service already connects as: a user unit,
`GLPI_DB_URL` of `mysql://pde@localhost/glpi`. The old narrow grants on
`glpi_logs` and `glpi_refusedequipments` still appear in `SHOW GRANTS` and are
now redundant.

*Superseded: this file used to say SQL was undecided for those two tables and
must not be reached for quietly. The restriction is lifted; the costs are not.*

It buys: the two gaps above stop being gaps, fleet-wide history becomes one
query instead of one per machine, and the next question of this shape needs no
new grant and no new decision.

It costs:

- **Entity permissions are bypassed.** A profile scopes what the API returns;
  a `SELECT` sees every entity. Nothing read this way may be presented as
  though it had been filtered.
- **The schema becomes an interface.** A column renamed upstream breaks a
  query at runtime, where the API would have dropped a field and left an
  `Option` blank. Every direct read needs the per-column care
  `glpi_refusedequipments` already gets — see the `db-read` tab.
- **No scoping to this app.** The grant belongs to the login account, so every
  process running as `pde` is a DBA: any shell, any editor plugin, any agent.
  `DROP DATABASE glpi` needs no password, and `mysql.global_priv` — every
  account's authentication string — is readable. Scoping it would mean a
  separate Unix user and a system unit; that has not been done.

What the socket plugin still gives: no password exists anywhere, and the
account cannot authenticate over TCP at all, because the plugin needs a peer
credential the kernel supplies only over a local socket. That is the whole of
the remaining protection, and it is about *reachability*, not authority.

## Writes: the blanket read-only rule was lifted 2026-11-26

The grant permits `INSERT`, `UPDATE`, `DELETE` and DDL on every database. The
old rule — "this app reads and must keep reading" — is gone, but its reasoning
still covers half the surface:

- **A table GLPI manages: never.** GLPI keeps its own history, entity scoping
  and derived state for its own tables, and an `UPDATE` behind it skips all
  three in ways the UI will not show. Writes that must land in GLPI's data go
  through the API or GLPI's own code.
- **A table GLPI does not manage: allowed.** A plugin's own `glpi_plugin_*`
  tables, for instance, where there is no history, no derived state and no
  entity scoping to bypass. Gate each behind its own `GLPI_INV_*_WRITES` flag.

## A database account does not help with the API

Settled in `docs/gl-db-api.md`: the endpoint list, each endpoint's field list,
the right → (tab, group) taxonomy, and which right an endpoint requires are
all PHP, not rows. And where a question *is* answerable outside v2.3, v1
answers it at lower cost — same account, same profile rights, same entity
scope, none of which SQL preserves. SQL is for where neither API answers.

## The `glpi_logs` grant was provisioning, not cruft

Its scope is moot since the grant above, but the mistake it records is still
available against any table nothing reads yet.

The grant was `SELECT` on `glpi_refusedequipments` **and** `glpi_logs`. Only
the first was read by any code, so an audit by present usage concluded the
second was cruft — twice, the second time argued in detail before being
caught. It is held for field-level change history, "what changed between
runs": a serial or MAC that silently changed between inventories is the
natural fourth quiet failure beside "stopped reporting", "never landed" and
"no agent", and squarely what this app is for.

**v1 can read that history**, correcting the first reason given for the grant,
which claimed no API could at any version. v2.3 models no `Log` schema at any
path, but v1's `getItem` takes `with_logs=true` and returns that asset's
`glpi_logs` rows, gated by the `logs` right this account already holds.
Measured on this install: `Computer/1?with_logs=true` returns 42 entries, 7 of
them field-level with real `old_value` → `new_value` pairs.

The gap is narrower and still real: v1 answers **one asset per request** — the
same per-machine cost that sent Hardware, Software and Network to v1
*collections*. Fleet-wide history in a single query is what the grant buys.

**The general rule, worth carrying beyond this grant: this app is still being
built, so what the code uses today is the wrong test.** The question is
whether the capability is on the way. The same error was made about GLPI 12's
new API surface before it was made here.

---

Part of the guidance in [CLAUDE.md](../../CLAUDE.md).
