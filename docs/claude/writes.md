# What this app writes

Nothing until 2026-11-15. Every write path, and the flag that gates it:

- `POST /api/mariadb/innodb/{name}` — `GLPI_INV_INNODB_WRITES` —
  see `mariadb-variables.md`
- `POST /api/db/restart`, running `scripts/db-restart.sh` —
  `GLPI_INV_DB_RESTART` — see `db-restart.md`
- `POST /api/mariadb/variables/{name}` and `…/persist` —
  `GLPI_INV_VARIABLE_WRITES` — see `mariadb-variables.md`
- the `glpi_inv` schema and its sampler — **no flag** — see below
- the php.ini tuning file — `GLPI_INV_PHP_WRITES` — see `php-tuning.md`

All flags are off by default, and a new write path gets a **new** flag rather
than a widening of an existing one. `POST /api/mariadb/engine` and its
`GLPI_INV_ENGINE_WRITES` existed for part of 2026-11-14 — see
[engine-page.md](engine-page.md) before reviving anything of that shape.

## The sampler, and why it has no flag

Landed 2026-11-21. The backend creates `glpi_inv` — its own schema — at
startup, with a
`status_samples` table and a `sample_status` EVENT that snapshots 16
`GLOBAL_STATUS` counters a minute and prunes past 14 days. It also sets
`event_scheduler = ON`, because an event that exists and never fires leaves an
empty table and reports nothing.

The other flags gate changes to something someone else owns — the server's
running configuration, its startup file, its lifecycle — where the blast
radius is the whole install. This creates one schema belonging to this app and
writes rows to it. A flag would also defeat it: off by default leaves every
fresh install with no sampler, which is the thing it exists to fix.

**Nothing here touches `glpi`.** A history table inside GLPI's schema would
have broken that rule for a dashboard's convenience; `glpi::sampler` has a
test asserting the generated SQL never qualifies anything with `glpi.`.

Worth not undoing:

- **It never fails startup.** The account may not hold CREATE, or this may be
  a server the app only reads. Six of the seven pages do not care, so a
  failure is logged and the history panel renders its "nothing is sampling"
  state — already a state it has, and a truthful one.
- **The event is generated from `SAMPLED_COUNTERS`**, not written out beside
  it. The counter list lived in two places for a day — an `IN` list on the
  server and the Rust const — and they drift silently. `StatusHistory::missing`
  reports that drift and is kept for the case it still catches: an event
  edited by hand on the server.
- **`IF NOT EXISTS` on the schema and table, `CREATE OR REPLACE` on the
  event.** Existing samples are never dropped; the event holds no state, only
  a schedule, so replacing it costs nothing and lets a changed counter list
  take effect on the next start without a migration.

---

Part of the guidance in [CLAUDE.md](../../CLAUDE.md).
