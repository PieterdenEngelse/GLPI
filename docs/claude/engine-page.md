# There is no engine page

A `maria` sub-tab with a strip of eight engine tabs, and a
`POST /api/mariadb/engine` behind `GLPI_INV_ENGINE_WRITES` running
`ALTER TABLE ... ENGINE=`: both existed for part of 2026-11-14 and were
removed the same day.
Do not rebuild either without reading this. Recoverable at `36d05b0` (the
tabs) and in CLAUDE.md's history (the write).

What the exercise established, all measured on this box:

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

---

Part of the guidance in [CLAUDE.md](../../CLAUDE.md).
