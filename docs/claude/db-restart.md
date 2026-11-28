# Restarting the database: the OS decides, and the script asks

`scripts/db-restart.sh` is the only thing that knows how, and it detects the
OS first because the answer differs completely. On NixOS the plain restart is
a polkit prompt away, while changing a startup-only setting cannot be done
from the app at all — `/etc/my.cnf` is a read-only store path generated from
`services.mysql`. On Debian/RHEL both work. On Windows the file is writable
but there is no polkit to ask with.

On NixOS that is a *write* it cannot do, not a restart it cannot do, and the
distinction matters: `mysql.service` carries `X-Restart-Triggers` naming
my.cnf's store path, so a changed setting gives a new path, a changed unit and
a restart performed by `nixos-rebuild switch` itself. The rebuild IS the
restart — nobody should click the restart link afterwards — and a restart
without a rebuild can never apply one of the 31. The script says so; it said
the opposite by omission until 2026-11-15.

`backend/src/db_restart.rs` only finds the script and returns what it said,
verbatim, with its exit code: 0 done, 2 refused, 3 not possible here, 4 tried
and rolled back.

Worth not undoing:

- **Neither link restarts on the first click.** The first runs with `DRY_RUN`
  and prints the commands; a second, separate click runs them. Previews are
  allowed even when the flag is off, so a disabled control can still explain
  itself.
- **`innodb_page_size`, `innodb_data_file_path` and `innodb_undo_tablespaces`
  are denied by name**, not by validation. They are valid values that leave
  the server unable to start against its existing datadir, and a config file
  keeps it that way across every later restart. No validation catches them
  because the value is not what is wrong:
  `mariadbd --help --verbose --innodb-page-size=64K` passes clean.
- **`mariadbd` has no `--validate-config`, and passing it starts a server.**
  That is a MySQL 8 flag. The unknown option is not reported until after the
  storage engines initialise, so the command writes a datadir and then aborts
  — measured at 145MB into `--datadir`, which was `/tmp`, and `DRY_RUN` did it
  too. Its own `[ERROR] unknown option` line then matched the script's error
  grep, so every valid setting came back as *rejected by the server*. Fixed
  2026-11-18: `validate_settings` uses `--help --verbose`, which parses the
  same options through the same code, writes nothing, and reports on stderr.
  `~/docs/gl-win-install.md` asserted the flag existed and gave a
  verification for it; that test used `innodb_page_size=7777`, which InnoDB
  refuses early enough that the real error looks like validation working.
- **106 of this server's 137 InnoDB settings never need any of this.** They
  are dynamic (`SYSTEM_VARIABLES.READ_ONLY = 'NO'`), so SQL changes them and
  the init-file keeps them. Only 31 need a file and a restart; a
  restart-shaped control is the wrong instrument for the other 106.

Full reasoning, per OS, with the measurements: `~/docs/gl-win-install.md`.

---

Part of the guidance in [CLAUDE.md](../../CLAUDE.md).
