# Session expiry: two php.ini keys, and the one that silently loses

GLPI has no session-duration setting of its own. That is the whole reason the
Logout tile exists: the figure everyone calls "the GLPI logout time" is PHP's,
and the figure GLPI does own, `login_remember_time`, is a cookie lifetime and
not a timeout.

Behind `GLPI_INV_PHP_WRITES`, pointed at `GLPI_INV_PHP_TUNING`. Same shape as
the nginx tuning file: one Nix file this app writes, imported by the system
config, applied by `nixos-rebuild switch` and never by the app — php.ini is a
read-only store path and PHP has no runtime settings interface, so there is no
apply-it-now at any price. `glpi::logout` owns it; the tile is its only reader.

Worth not undoing:

- **A key `glpi.nix`'s `php.buildEnv { extraConfig }` already sets cannot be
  written from here, and the failure would be silent.** The phpfpm module
  builds the pool's `-c` ini as `cat ${phpPackage}/etc/php.ini
  $phpOptionsPath`, then PHP scans `${phpPackage}/lib`, where `buildEnv` puts
  its own php.ini — and a scanned file is loaded last and wins. Measured by
  appending to a copy of this pool's real `-c` ini and reading it back through
  the pool's own php binary:

      session.gc_maxlifetime = 28800   ->  28800   (absent from extraConfig)
      memory_limit = 333M              ->  256M    (extraConfig sets it)
      session.cookie_httponly = 0      ->  On      (extraConfig sets it)

  So `Owner::Package` refuses those keys rather than writing a file that
  rebuilds clean and changes nothing. Corollary: do **not** tidy
  `session.gc_maxlifetime` into that `extraConfig`, which would turn the
  control into one of them.

- **`login_remember_time` is read and never written.** It is a row in
  `glpi_configs`, which GLPI manages — history, cache in `$CFG_GLPI` — so the
  rule in [db-access.md](db-access.md) sends it through GLPI's own code, and
  GLPI's API has no config write at any version (`getGlpiConfig` reads;
  nothing writes). The row carries a link into GLPI's General setup instead of
  a field. It is read over **v1** `getGlpiConfig`, not SQL: `getSafeConfig()`
  strips only the eight `Config::$undisclosedFields` secrets, and this is not
  one.

- **`session.gc_maxlifetime` has a 60-second floor.** PHP reads 0 as "eligible
  for collection immediately" and `CronTask::cronSession` reads the same 0 as
  "fall back to a week", so at 0 the two halves of one setting disagree about
  what was asked for.

---

Part of the guidance in [CLAUDE.md](../../CLAUDE.md).
