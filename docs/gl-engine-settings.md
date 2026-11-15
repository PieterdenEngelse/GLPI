# MariaDB's settings, and what can actually be changed — measured on 11.4.12

What it takes to change an InnoDB setting on this deployment, and which ones
cannot be changed at all. Everything marked **measured** was observed against
the running server or a throwaway instance of the same build on 2026-11-14 and
2026-11-15; nothing here is taken from documentation, and two things in it
contradict what the documentation would have led you to write.

Written while building the InnoDB panel's controls, the startup file behind
them, and the Perf page. Those landed inside commit `690726c`, whose message
describes a different change — this file is the record they would otherwise
not have.

- **Does MariaDB persist `SET GLOBAL`?** No — no `SET PERSIST`, no
  `mysqld-auto.cnf`, no persistence option in the binary at all — measured
- **How many settings can a running server take?** 106 of 137 — measured
- **Can a script change the other 31 and restart?** Not on this box — three
  independent locks, all root — measured
- **Does converting an engine silently drop foreign keys?** No, on this
  version — the ALTER is refused outright — measured, and it corrects a claim
  that was made here first

---

## The 106 and the 31

`information_schema.SYSTEM_VARIABLES` answers this per setting, and it is the
only honest source: `READ_ONLY = 'YES'` marks the ones taken at startup.

    SELECT READ_ONLY, COUNT(*) FROM information_schema.SYSTEM_VARIABLES
     WHERE VARIABLE_NAME LIKE 'INNODB%' GROUP BY READ_ONLY;
    NO   106
    YES   31

The same table carries `VARIABLE_TYPE`, `ENUM_VALUE_LIST`, `NUMERIC_MIN_VALUE`,
`NUMERIC_MAX_VALUE` and `NUMERIC_BLOCK_SIZE`, which is why the panel's controls
are built from it rather than from a table in this app: the type decides which
control a row gets, the enum list and range decide what it may offer, and a
list written here would rot at the next upgrade with nothing to notice.

**The value is read back after every write.** The server rounds to
`NUMERIC_BLOCK_SIZE`: asking `innodb_buffer_pool_size_auto_min` for
8796093022207 stores 8796084633600 — measured. Echoing the request would have
been a lie the page told confidently.

## Persistence is a file, and only a file

`SET PERSIST innodb_print_all_deadlocks=ON` → `ERROR 1064` (syntax error).
`mariadbd --help --verbose` has no persistence option of any kind, and no
`mysqld-auto.cnf` exists or can. **Measured.** That is MySQL 8's feature and
MariaDB never took it.

So anything that outlives a restart is in a file the server reads at startup,
and on this deployment there is exactly one:

- `/etc/my.cnf` → `/etc/static/my.cnf` → a Nix store path. Immutable.
- The unit runs `mysqld --defaults-file=/etc/my.cnf`, which **replaces** the
  search path: `/etc/mysql/my.cnf` and `~/.my.cnf` are never read.
- `--defaults-extra-file` is **ignored when `--defaults-file` is given** —
  measured with `--print-defaults`, which returns only `bind-address`,
  `datadir` and `port` with both flags, and the extra file's contents when it
  is used alone. So there is no second file to drop beside the first.

### `--init-file`, and its one sharp edge

For the 106, SQL replayed at startup does persist them, before any client
connects. Measured on a throwaway instance of the same build:

| init file | server | result |
|---|---|---|
| bad line in the middle | starts | line 1 applied, **line 3 never ran** |
| bad line last | starts | both earlier lines applied |

A failing statement does not stop the server — it logs `ERROR: 1193` and
startup continues — but **the rest of the file is skipped**. One setting a
later MariaDB no longer understands therefore drops every line below it, and
only the error log says so. There is no `--force` and no per-statement
continue, so the panel compares the file against the running server and shows
the mismatch. That comparison is the only defence available.

**A missing or unreadable init-file aborts startup**: `Can't create/write to
file … (Errcode: 2)` then `Aborting`, exit 1 — measured. Not a setting that
quietly fails: a database that does not come back. Hence the file is written
0644 explicitly rather than at the mercy of umask, dropping the last setting
rewrites it with only the header instead of deleting it, and the one-time
NixOS step must create the file before the rebuild:

```nix
services.mysql.settings.mysqld.init-file = "/var/lib/glpi-inv/innodb.sql";
systemd.tmpfiles.rules = [
  "d /var/lib/glpi-inv 0755 pde users -"
  "f /var/lib/glpi-inv/innodb.sql 0644 pde users -"
];
```

It cannot live under `/home`: the unit sets `ProtectHome=true`. `/var/lib` is
readable — `ProtectSystem=strict` blocks writes, not reads — and `/tmp` is not,
because `PrivateTmp=true` gives mysqld its own.

## The 31 cannot be automated here, and that is NixOS

Three independent locks, each measured:

1. `/etc/my.cnf` is a store path.
2. `--defaults-extra-file` is ignored, so nothing can be bolted on beside it.
3. `/etc/systemd/system` is a symlink into `/etc/static` and read-only, so
   there is no unit drop-in; the start script's only variable parts are
   `$_WSREP_NEW_CLUSTER` and `$_WSREP_START_POSITION`, which come from unit
   `Environment=`; and restarting `mysql.service` is root.

What does work is the declarative route, and it is one action rather than two:
`services.mysql.settings.mysqld.<option>` plus `nixos-rebuild switch`. The
option type is freeform — `nix eval` reports *attribute set of section of an
INI file* — so any mysqld option is legal there. The unit's
`X-Restart-Triggers` **is the my.cnf store path**, so the rebuild restarts
MariaDB into the new value by itself; without that trigger the file would
change and the running server would keep the old values until someone noticed.

## Engine conversion refuses; it does not silently drop keys

This file's first version said converting an InnoDB table to Aria or MyISAM
silently drops declared foreign keys. **That is older MySQL and MariaDB
behaviour and not this build's.** With one real constraint in place, 11.4.12
answers `ERROR 1217` to all four of: child → MyISAM, referenced parent →
MyISAM, child → Aria, and child → MyISAM with `foreign_key_checks=0`. Both
tables were still InnoDB afterwards. **Measured**, in a scratch schema that was
dropped after.

It is moot for GLPI, which declares no foreign keys at all, but the correction
matters in the direction it runs: the wrong version makes a reader expect
silent damage where the server in fact refuses outright.

## Where GLPI's time actually goes

The Perf page exists because the settings above invite tuning the wrong thing.
Measured on this install:

| | median |
|---|---|
| `/status.php` | ~60 ms |
| `/index.php` (login page, a real render) | ~50 ms |
| `COUNT` over the largest inventory table | **0.55 ms** |

A login page costs about **90×** the query inside it. Buffer pool hit rate is
99.75% (905 421 requests, 2 268 from disk) on a 50.8 MiB database under a
128 MiB pool — the whole thing is in memory, so no InnoDB setting here can be
felt. The ratio, not either number, is what to watch: it closes as the data
outgrows memory, and that is when the engine's settings start deciding how fast
GLPI feels.

One figure on that page is worth a second look and is nothing to do with
InnoDB: **19.9% of temporary tables spilled to disk** (7 252 of 36 440), which
is `tmp_table_size` and `max_heap_table_size` territory.

## The decisions these measurements settled

- **Controls are built from the server's metadata**, never from a list here.
- **Validation happens server-side at write time**, re-reading whether the
  setting exists and may be written; the statement is built from the matched
  enum entry or the parsed number, never from submitted text.
- **Writes are off unless `GLPI_INV_INNODB_WRITES=1`**, checked in the handler
  rather than only in the UI.
- **A value good enough to apply now is the only kind worth writing into a
  file nobody will be watching when it runs**, so persistence goes through the
  same validation as the live write.
- **No verdicts on the Perf page.** A hit rate that is excellent on one
  workload is a symptom on another, and a threshold written into this app
  would be its opinion wearing the server's clothes.
