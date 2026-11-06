# A read-only database account for glpi-inv — what it would take

The two things neither GLPI API exposes are `RefusedEquipment` and
`glpi_logs`. `gl-db-api.md` settled that a database account helps with
nothing *about* the API; this file is the other half — what it would take to
reach those two, and what it costs. Columns below were read from the GLPI
11.0.12 source, not assumed.

**Not built. This is a proposal.**

- **Worth it for:** refused submissions, the failure this app most wants
- **Not worth it for:** anything the API already answers
- **Cost:** schema coupling across upgrades; a second access path
- **Avoidable cost:** no new password, if the account uses socket auth

---

## What the two tables hold

### `glpi_refusedequipments` — the one that matters

An agent submits an inventory, a rule rejects it, the agent exits 0, and no
asset ever appears. Nothing in the agent's log says so and neither API can
see it. Columns, from `src/RefusedEquipment.php`:

| column | use on the page |
|---|---|
| `date_creation` | when the submission was refused |
| `name` | what the machine called itself |
| `itemtype` | what it tried to become — usually Computer |
| `serial`, `uuid` | identity, for matching against an existing asset |
| `ip`, `mac` | where it came from |
| `method` | how it submitted |
| `rules_id` | **which rule refused it** — the actionable field |
| `entities_id` | which entity it was aimed at |

### `glpi_logs` — field-level history

| column | use |
|---|---|
| `itemtype`, `items_id` | what changed |
| `id_search_option` | which field |
| `old_value`, `new_value` | the change |
| `date_mod`, `user_name` | when, and by whom or what |

Lower value and much higher volume: this table grows without bound on a busy
GLPI, so any query needs a date bound and a LIMIT from the first line.

## What the pages would show

**Refused** — a page beside Agents and Computers, answering the third failure.
Agents asks which agents went quiet; Computers asks which machines have no
agent; this asks **which submissions GLPI threw away**. Most recent first,
with the refusing rule named. A machine appearing here *and* in Computers is
a different story from one appearing only here: the first is an update that
was rejected, the second never became an asset at all.

**History** — probably not its own page. Better as a per-agent or per-computer
panel: "what changed on this machine, and when". `id_search_option` has to be
resolved to a field name, which is a lookup this app does not have today.

## The account

The useful trick: **no new password.** MariaDB can authenticate by Unix socket,
matching the OS user. The service runs as `pde`, so:

```sql
CREATE USER 'pde'@'localhost' IDENTIFIED VIA unix_socket;
GRANT SELECT ON glpi.glpi_refusedequipments TO 'pde'@'localhost';
GRANT SELECT ON glpi.glpi_logs            TO 'pde'@'localhost';
FLUSH PRIVILEGES;
```

Two tables, SELECT only, no secret to store, and nothing to leak if the
dashboard's config is read. It is also exactly the grant `CLAUDE.md` named as
the acceptable form of this.

Needs deciding: `rules_id` is a number. Showing the rule's *name* means either
`GRANT SELECT ON glpi.glpi_rules` — a third table — or displaying the id and
linking into GLPI. **Prefer the link.** The grant stays at two tables and the
page stays honest about where the answer lives.

Entity names are already available from the API (`/api/entities`), so
`glpi_entities` is not needed.

## The cost, stated plainly

**Schema coupling.** A renamed column breaks this silently, where the API
would have named the missing field. Mitigation: treat every column as
optional and render a blank, the way the frontend already treats backend
fields.

**Two notions of "everything" on one screen.** API rows are entity-filtered by
GLPI; SQL rows are not. For an app whose whole premise is that absence means
something, that is the real hazard — not privilege. Mitigation: filter SQL
results by the entity ids the API already reports as visible, so both halves
of the page mean the same thing, and say so in the scope line.

**A second access path.** More to go wrong, more to explain. It is justified
only because the alternative for these two tables is nothing at all.

## Not a cost, despite appearances

**Privilege.** SELECT on two tables is narrower than what the `inv` API account
already holds — that account can read every computer, agent, profile and user
in the install.

**Trap: the table is `glpi_refusedequipments`, plural.** GLPI pluralises table
names, so a grep for the singular matches every occurrence of the plural as a
prefix and reads back as confirmation. That mistake reached a GRANT, which
failed with `ERROR 1146 ... Table 'glpi.glpi_refusedequipment' doesn't exist`.

## Unknowns

- Whether `glpi_refusedequipments` has any rows here. This install has one
  agent and no known refusals, so the page may be empty for a long time —
  which is the correct outcome and a poor test.
- MariaDB currently listens on `0.0.0.0:3306`, not loopback. That is wider
  than this needs and is worth narrowing regardless of whether this proposal
  goes ahead.
- Whether socket auth works for a user that is not the DB owner on this
  install; `unix_socket` is configured for `glpi` already, so the plugin is
  present, but a second socket user has not been tried.
