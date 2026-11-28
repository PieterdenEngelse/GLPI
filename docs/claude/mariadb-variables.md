# Writing MariaDB variables, and making it outlive a restart

`POST /api/mariadb/variables/{name}` is the general case of the InnoDB write,
behind its own `GLPI_INV_VARIABLE_WRITES` since 2026-11-20. **It is not a
widening of the InnoDB flag and must not be folded into it**: that one admits
106 settings of one subsystem, this one every dynamic variable the server has
— 484 of this server's 693. The other 209 are 196 the server refuses at
runtime and 13 that are SESSION ONLY with no global value at all. `settable`
on each row says which, and it is **not** `!read_only`: the live tier is cut
on `READ_ONLY` alone, so those 13 sit in it and cannot be written.

Worth not undoing:

- **The validation was never InnoDB-specific.** `literal_for` builds its
  literal from `VARIABLE_TYPE`, `ENUM_VALUE_LIST` and the numeric bounds
  `SYSTEM_VARIABLES` publishes for every row, and the name in the SQL is the
  server's own spelling of a row it just reported. Only the `LIKE 'INNODB%'`
  filter made the old write InnoDB-shaped. Keep the literal built from a
  parsed number or a matched enum entry; never from submitted text.
- **`glpi::hazard` is nine names that are valid to set and break something
  else** — the connection limit that locks everyone out, the buffer pool this
  host has not the memory for, the character set GLPI's tables are created
  with. They answer `409` with the reason and are set only on a second request
  carrying `confirm`. It is deliberately **not** a deny list; the deny list is
  `db-restart.sh`'s `DENIED`, and the distinction is recoverability — nothing
  in `hazard` leaves a server that cannot start, and everything in it is
  recoverable only *from* a running one.
- **`409` rather than `400` for the confirm case.** "Come back and mean it"
  and "the server will not take that value" look identical at 400 and need
  opposite things from the reader.

## Making a change outlive a restart

**Setting a variable keeps it.** The endpoint writes the running value *and*
the startup-file line, in that order. These were separate endpoints at first
and the split was wrong in practice: a value set and not kept reverts at the
next restart with nothing on the row saying it will, so the control that
appeared to work was the one that silently did not last. Un-keeping is now the
deliberate act, which is the right way round — it is the reversible one.

A failure to keep does **not** fail the request. The server has already taken
the value, and an error would describe a write that happened as one that did
not. It comes back as `kept: false` and the row says so. (`set_innodb` is
deliberately unchanged — it has its own flag and its own panel, and widening
its behaviour is not what that flag was granted for.)

`POST /api/mariadb/variables/{name}/persist` remains for the two things
setting cannot express: keeping a value you did not change, and dropping a
line without touching the running server. A request with no body drops it. It
goes through the same `variable_literal` gate as the write, and that is not
tidiness: a line the server refuses at startup **silently drops every line
below it** — measured; the server logs the error, startup continues, the file
stops executing there. So a value that could not be set must not be
persistable either.

`glpi::init_file`, named `innodb_init` until 2026-11-20, owns the file for
both the InnoDB panel and the variables pages. One file, one format; only what
`SET GLOBAL` can set may go in it.

**The root setup is done on this machine** — directory, file, `my.cnf` entry
and `GLPI_INV_INIT_FILE` all exist, so `persist_path` is set and the pages say
a change outlives a restart. Kept here because it is what a new machine needs,
and because the three traps below made it take two attempts:

```nix
services.mysql.settings.mysqld.init_file = "/var/lib/glpi-inv-init/persist.sql";
systemd.tmpfiles.rules = [
  "d /var/lib/glpi-inv-init 0750 pde mysql -"
  "f /var/lib/glpi-inv-init/persist.sql 0640 pde mysql -"
];
```

then `GLPI_INV_INIT_FILE` — renamed from `GLPI_INV_INNODB_INIT_FILE` —
pointing at the same path.

- **It cannot live under `/home`.** The mysql unit runs `ProtectHome=true`, so
  the server cannot see it there.
- **The file must exist before MariaDB is pointed at it.** An init-file mysqld
  cannot open does not quietly fail: it logs
  `Can't create/write to file … (Errcode: 2)`, then `Aborting`, and exits 1.
  Hence the `f` tmpfiles rule and not just the `d`.
- **A drop-in cannot be used instead.** mysqld runs
  `--defaults-file=/etc/my.cnf`, which replaces the whole search path, so
  `--defaults-extra-file` and `/etc/my.cnf.d` are both dead ends. The only
  routes are this file and `services.mysql.settings`.

---

Part of the guidance in [CLAUDE.md](../../CLAUDE.md).
