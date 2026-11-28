# Why this app speaks two GLPI APIs

v2.3 for the asset-level view — agents, computers, entities, session. **v1
(`/apirest.php`) for rights and authorizations, and for the three inventory
detail pages**, in `glpi/legacy.rs` and `glpi/inventory.rs`.

`just probe <path>` hits v2.3; `just probe-v1 <path>` hits v1. The
endpoint-to-right mapping and fifteen traps are in `docs/gl-rights.md`.

## v1 for rights and authorizations

Verified by reading `src/Glpi/Api/HL/`:

- **`Profile_User` has zero occurrences anywhere under HL.** Who may use which
  profile, on which entity, and whether that is recursive, is not modelled.
- **HL's `Profile` schema carries neither `interface` nor any rights** — only
  `id`, `name`, `comment`, `is_default`, `helpdesk_hardware`,
  `helpdesk_item_type`, `managed_domainrecordtypes`, `mfa_enforced`.

So v2.3 can describe the session it is already in and nothing else: it can say
"something is missing" but never "this profile, on this entity, is the one to
fix". That question is the whole point of the Status and Rights pages.

**This is a different trade from the MariaDB one in
[db-access.md](db-access.md) and must not be confused with it.** v1 does not
bypass GLPI's permissions — every call is still made as the service account
and still filtered by its profile, which is the precise objection to going
direct to SQL. What it costs is stability: GLPI calls v1 "closely tied to the
GLPI source code ... no promise of stability between versions", and it needs
its own authentication because it rejects the OAuth bearer outright.

## v1 for hardware, software and network

The three pages under `/inv` are fleet-wide and read v1 collections. Verified
by reading `src/Glpi/Api/HL/Controller/` and by probing the running install:

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

`Legacy::get_paged` walks these. `Legacy::get` — one generous range, refuses
anything partial — is still what the bounded rights tables use: a single NixOS
machine holds 1468 `Item_SoftwareVersion` rows, so no fixed range is
defensible for inventory.

Scope comes from v2.3 regardless: every inventory page is filtered to the
computers `Assets/Computer` returns, so all pages mean the same thing by
"every machine" even though they read different APIs.

---

Part of the guidance in [CLAUDE.md](../../CLAUDE.md).
