# GLPI 11.0.12 — API rights for a read-only service account

What a dedicated service account needs to read inventory telemetry through
GLPI's APIs, how to read its rights back out again, and the traps along the
way. Everything marked **verified** was either read in the GLPI 11.0.12 source
in `/nix/store/…-glpi-11.0.12/` or observed against the live instance on
`localhost:8080`; nothing here is inferred from documentation.

Mostly the High-Level API (v2.3). The legacy v1 API appears only where v2.3
cannot answer at all, which is flagged where it happens.

Written while switching `glpi-inv` off the built-in `glpi` super-admin and
onto a scoped account, which is what surfaced all of it.

- **The rights needed:** `agent`, `entity`, `computer`, `profile`, `user`, all
  READ — verified (was two; the app grew — see Minimum viable service account)
- **Where they live in the UI:** Profile → Administration — verified
- **What the 403 tells you:** nothing — verified
- **Whether recursion can be audited via the API:** no — verified
- **Whether a profile's rights can be read via the API:** yes, all of them, but
  only through v1 — verified
- **Whether the tab/group structure of the rights form can be:** no, by any
  route — verified

---

## The two core rights

The two the inventory telemetry itself cannot work without. The full list for
`glpi-inv` as it stands is under *Which right an endpoint needs*; these two are
where the surprises live.

| endpoint | right | source |
|---|---|---|
| `GET /api.php/v2.3/Inventory/Agent` | `agent` | `Agent::$rightname = 'agent'` — src/Agent.php:70 |
| `GET /api.php/v2.3/Administration/Entity` | `entity` | `Entity::$rightname = 'entity'` — src/Entity.php:70 |

READ on both. Nothing else: an app that only reads needs neither UPDATE nor
PURGE, both of which the profile form offers on the same row.

**Trap 1 — the right is not named after the path.** The endpoint is
`Inventory/Agent`, so `inventory` is the obvious guess, and it is wrong.
src/Agent.php makes this explicit:

```php
public static $rightname = 'agent';
//static $rightname = 'inventory';
```

The commented-out line is upstream's own abandoned alternative, sitting
directly beneath the live one. There *is* a separate `inventory` right in the
profile form (labelled "Inventory", Profile.php) and granting it does nothing
for this endpoint.

## Where they are in the UI

**Administration → Profiles → <profile> → Administration tab.** Both are
registered in the same block of `Profile::showForm`:

```php
$fn_get_rights(Entity::class, 'central', ['scope' => 'global']),   // :978
$fn_get_rights(Agent::class,  'central', [...]),                   // :1029
```

**Trap 2 — a Simplified-interface profile cannot hold these rights at all.**
Note the `'central'` argument above. Profile.php offers two different tab
sets depending on the profile's interface:

```php
// helpdesk interface                 // central interface
2 => $item->showFormAsset(),          2 => $item->showFormAsset(),
3 => $item->showFormTrackingHelpdesk(),
6 => $item->showFormToolsHelpdesk(),  …
                                      7 => $item->showFormAdmin(),
```

The helpdesk set has no Administration entry. If the tab is missing, the
profile's interface is Simplified — not a missing right, a wrong interface,
and no amount of looking for the checkbox will help.

## What the failure looks like

**Verified live.** With the account authenticating but unauthorised:

```json
{"status":"ERROR_RIGHT_MISSING",
 "title":"You don't have permission to perform this action.",
 "detail":null}
```

HTTP 403, on both endpoints independently.

**Trap 3 — `detail` is null, so GLPI never names the missing right.** The
message is identical whichever of the two is absent, and identical again for
any other right. The only way to tell `agent` from `entity` is which endpoint
failed, which is why it is worth calling them separately rather than folding
the entity lookup into the agent request.

Distinguishing it from the neighbouring failure modes:

| symptom | cause |
|---|---|
| 401 at the token endpoint | wrong username/password, or `client_credentials` instead of the `password` grant |
| 401 on a data endpoint with a valid token | token minted by `client_credentials` — Router.php:717 requires a logged-in user, and that grant has none |
| 401 after working for a while | the user session ended independently of the JWT clock (restart, session GC) |
| **403 ERROR_RIGHT_MISSING** | **authenticated, but the profile lacks the right** |
| 200 with fewer rows than expected | entity filtering — see below |

## Entity scope, and why it cannot be audited

`Search.php:570-574` appends an entity restriction to every collection query:

```php
$criteria['WHERE'][] = ['AND' => $entity_restrict];
```

**Trap 4 — filtered rows leave no trace in the response.** Compare the two
ways GLPI can return less than everything:

- **Pagination is honest.** `ResourceAccessor.php:386-393` returns 206 plus
  `Content-Range: start-end/total`, so a client can see what was withheld.

  **Correction.** This file previously claimed, as verified, that with no
  parameters there is no cap at all. That is false. v2.3 caps a collection at
  **100 rows** by default. The error came from testing only collections
  smaller than the cap — 9 profiles, 1 computer, a handful of agents — where
  a complete answer and an uncapped one look identical. Measured on a
  1330-row collection:

  ```
  Assets/Software                     ->  206  Content-Range: 0-99/1330
  Assets/Software?limit=500           ->  206  Content-Range: 0-499/1330
  Assets/Software?start=1300&limit=100 -> 200  Content-Range: 1300-1329/1330
  ```

  `limit` is honoured rather than clamped, and the status distinguishes a
  partial window (206) from one reaching the end (200). `glpi-inv` walks
  collections with `start`/`limit` until the window reaches the total; before
  that it sent no parameters and refused any 206, which would have failed
  outright on any install holding more than 100 agents or computers.
- **Entity filtering is silent.** The excluded rows never enter the `total`,
  so there is no number to compare against. Completeness is **unverifiable
  from inside the response.**

For an app whose purpose is noticing which machines stopped reporting, that is
the worst available failure: an agent outside scope is absent, and absent is
indistinguishable from decommissioned.

Two separate settings control this, and they are easy to conflate:

- **`entity` READ** — a *global* right (`'scope' => 'global'`, Profile.php:978)
  governing whether the account may list entities at all.
- **`is_recursive` on the account's authorization** — governs whether it sees
  entities *below* the one it is attached to, and therefore their agents.

Granting the first does not grant the second.

### Reading `is_recursive` in the UI

**Administration → Users → <user> → Authorizations tab.**

**Trap 5 — the tab is named differently depending on which form you are on,
and `getTypeName()` gives the wrong answer for the user form.**
`Profile_User::getTypeName()` returns `_n('Profile', 'Profiles')`, which is
what the class is called on the Entity and Profile forms. The User form does
not use it — `getTabNameForItem` has a separate `case User::class` returning
`_n('Authorization', 'Authorizations')`. Looking for "Profiles" on a user
page finds nothing.

Each row carries a bold suffix built at Profile_User.php:195-206 and
:368-379:

| suffix | meaning |
|---|---|
| `(R)` | recursive — child entities included |
| `(D)` | dynamic — granted by an LDAP rule |
| `(D, R)` | both |
| *none* | **not** recursive — this entity only |

So `Read-Only (R)` is correct and `Read-Only` is not.

**Diagnostic: the "Default profile" dropdown on the user form lists only
profiles the user already holds an authorization for** —
`Profile_User::getUserProfiles()` selects from `glpi_profiles_users WHERE
users_id = <id>`. So an empty dropdown means no authorization at all, and a
dropdown offering only Self-Service means that is the single authorization
present. Either reads as "this account can log in and see nothing", which is
exactly the 403 above.

**Not auditable through the API — verified.** `GET Administration/User`
exposes only `default_entity` and `default_profile` per user. The
`glpi_profiles_users` rows that actually carry `entities_id` and
`is_recursive` are not exposed, so recursion can only be checked in the UI or
the database.

## Adding the authorization — two things that mislead

The form is `templates/pages/admin/add_profile_authorization.html.twig`.

**Trap 6 — "Recursive" is a Yes/No dropdown, not a checkbox, and it defaults
to No.**

```twig
{{ fields.dropdownYesNo('is_recursive', 0, __('Recursive'), field_options) }}
```

There is nothing to tick. Leaving it alone silently gives a non-recursive
authorization, which is the one that quietly costs rows later.

The Entity dropdown defaults to `session('glpiactive_entity')` — whatever
entity you are currently working in, not necessarily the root — so it is
worth reading rather than accepting.

**Trap 7 — the Profile dropdown is filtered, and a user is not a profile.**

```twig
'condition': { 'WHERE': call('Profile::getUnderActiveProfileRestrictCriteria') }
```

`Profile::getUnderActiveProfileRestrictCriteria()` (src/Profile.php:688)
returns every profile if the logged-in user can create profiles — so a
super-admin sees all of them, and a profile missing from the list simply does
not exist. Otherwise it restricts to profiles whose `interface` matches the
current one *and* whose every right is a subset of the current profile's, so a
less-privileged admin cannot grant more than they hold.

Separately: creating a user does **not** create a profile of the same name.
Profiles are independent objects, and the dropdown lists profiles. Looking for
a profile named after the account finds nothing.

## Reading a profile's rights back through the API

Granting a right is one problem; auditing what an account actually holds is a
separate one, and the two APIs answer it very differently.

| route | gives | verdict |
|---|---|---|
| `GET /api.php/v2.3/Administration/Profile` | `id`, `name`, `comment`, `is_default`, `helpdesk_hardware`, `helpdesk_item_type`, `managed_domainrecordtypes`, `mfa_enforced` — AdministrationController.php:645-699 | lists profiles, **no rights at all** |
| `ProfileRight` under v2.3 | — | **no route exists** |
| `GET /api.php/v2.3/session` | `active_profile.rights`, from API version 2.2 up — CoreController.php:625-634 | real rights, **active profile only** |
| `GET /apirest.php/ProfileRight?range=0-9999` | one row per profile per right: `{id, profiles_id, name, rights}` | **every profile, every right** — 899 rows over 9 profiles here |

So auditing more than the current session means v1. That is the only reason
`glpi-inv` speaks v1 at all.

**Trap 8 — the `GLPI-Profile` header looks like it fixes that, and silently
does not.** Router.php:780-783 accepts a header naming another profile:

```php
if ($request->hasHeader('GLPI-Profile')) {
    $requested_profile = $request->getHeaderLine('GLPI-Profile');
    if (is_numeric($requested_profile)) {
        Session::changeProfile((int) $requested_profile);
```

and `Session::changeProfile` (Session.php:592-598) acts only on a profile the
account is authorised for:

```php
public static function changeProfile($ID)
{
    if (
        isset($_SESSION['glpiprofiles'][$ID])
        && count($_SESSION['glpiprofiles'][$ID]['entities'])
    ) {
```

**There is no `else`.** An unauthorised id, or one that does not exist, leaves
the session on its own profile and returns 200 with *that* profile's rights.
Verified live, as the `inv` account (authorised for 1 and 9 only):

| requested | returned | granted |
|---|---|---|
| 1 Self-Service | 1 Self-Service | 9 |
| 9 inv | 9 inv | 89 |
| **4 Super-Admin** | **9 inv** | 89 |
| **8 Read-Only** | **9 inv** | 89 |
| **999 (nonexistent)** | **9 inv** | 89 |

A caller that trusts its own request therefore prints the service account's
rights under the name of whichever privileged profile it asked about — wrong in
the most reassuring direction, and with nothing in the response to catch it.
Compare the id in `active_profile.id` against the one requested, or use v1.

**Trap 9 — `active_profile.rights` is not a list of rights.**
CoreController.php:626-633 walks every column of the profile row and zeroes
whatever is not a right:

```php
$all_right_names = array_keys(ProfileRight::getAllPossibleRights());
foreach ($active_profile as $key => $value) {
    if (in_array($key, $all_right_names, true)) {
        $session['active_profile']['rights'][$key] = (int) $value;
    } else {
        $session['active_profile']['rights'][$key] = 0;
    }
}
```

Verified live: 122 keys, of which 101 are rights. The other 21 are ordinary
columns presented as rights holding zero, which reads as "this permission is
denied":

```
2fa_enforced, change_status, changetemplates_id, comment,
create_ticket_on_login, date_creation, date_mod, entities,
helpdesk_hardware, helpdesk_item_type, id, interface, is_default,
last_rights_update, managed_domainrecordtypes, name, problem_status,
problemtemplates_id, ticket_status, tickettemplates_id, use_mentions
```

v1's `ProfileRight` returns rights and nothing else, so the count there is the
honest denominator.

**Trap 10 — a missing `ProfileRight` row is not a zero, and the row counts
differ per profile.** `glpi_profilerights` is four columns —
`id, profiles_id, name, rights` — and GLPI stores only the rows that exist.
Verified live: 101 distinct right names across the install, but

| profiles | rows | missing |
|---|---|---|
| 3, 4, 8, 9 | 101 | — |
| 1, 2, 5, 6, 7 | 99 | `rule_change`, `rule_problem` |

Counting each profile against its own rows gives them different denominators,
so "9 of 99" and "100 of 101" are not the same measurement. Use the union of
all names for every profile.

An absent row still denies — `Session::haveRight` (Session.php:1498-1502):

```php
if (isset($_SESSION["glpiactiveprofile"][$module])) {
    return (int) $_SESSION["glpiactiveprofile"][$module] & $right;
}
return false;
```

So absent and zero have identical effect, but they are different statements: a
zero is a decision taken in the profile form, an absent row is a right that
form never offered for that profile. Worth distinguishing on screen; not worth
distinguishing when deciding whether a call will work.

## A right is a bitmask

Twelve bits are defined globally — constants.php:66-78:

| bit | name | | bit | name |
|---|---|---|---|---|
| 1 | `READ` | | 64 | `UPDATENOTE` |
| 2 | `UPDATE` | | 128 | `UNLOCK` |
| 4 | `CREATE` | | 256 | `READ_ASSIGNED` |
| 8 | `DELETE` | | 512 | `UPDATE_ASSIGNED` |
| 16 | `PURGE` | | 1024 | `READ_OWNED` |
| 32 | `READNOTE` | | 2048 | `UPDATE_OWNED` |

`ALLSTANDARDRIGHT` (31, constants.php:71) is the OR of the first five, not a
thirteenth bit — decode it as five or it double-counts.

So `entity = 33` is `READ|READNOTE`, and `computer = 1313` is
`READ|READNOTE|READ_ASSIGNED|READ_OWNED` — four read bits and no write.

**Trap 11 — not every bit can be named, and the names are in no API.** Parts of
GLPI define further bits for things that exist only there. Observed live on the
`inv` profile:

| right | value | beyond the 12 |
|---|---|---|
| `ticket` | 138241 | 135168 |
| `problem`, `change`, `project` | 132129 | 131072 |
| `task`, `followup`, `knowbase` | 8193 | 8192 |

Those are real permissions. GLPI resolves their labels from
`Profile::getRightsForForm()`, a form definition reachable through neither API,
so a client can only report the leftover as a number. Dropping it instead would
understate what the account can do.

## Which right an endpoint needs

HL authorises a collection with `$itemtype::canView()`
(ResourceAccessor.php:353), and each schema names its itemtype in
`x-itemtype`. So the chain is endpoint → itemtype → `$rightname`:

| endpoint | API | right | source |
|---|---|---|---|
| `Inventory/Agent` | v2.3 | `agent` | `Agent::$rightname` |
| `Assets/Computer` | v2.3 | `computer` | `Computer::$rightname` |
| `Administration/Entity` | v2.3 | `entity` | `Entity::$rightname` |
| `session` | v2.3 | *none* | reports the caller's own session |
| `Profile` | v1 | `profile` | `Profile::$rightname` |
| `ProfileRight` | v1 | `profile` | see Trap 12 |
| `Profile_User` | v1 | `user` **and** `profile` | see Trap 12 |
| `Entity` | v1 | `entity` | `Entity::$rightname` |
| `getFullSession` | v1 | *none* | reports the caller's own session |

**Trap 12 — a child or relation endpoint needs its parents' rights, and a
relation needs both.** Neither `ProfileRight` nor `Profile_User` declares a
`$rightname`, so there is nothing to look up and no right named after either.

`ProfileRight` is a `CommonDBChild` of Profile, and CommonDBChild.php:130-136
falls through to the parent:

```php
public static function canView(): bool
{
    if ((static::$rightname) && (!Session::haveRight(static::$rightname, READ))) {
        return false;
    }
    return static::canChild('canView');
}
```

`Profile_User` is a `CommonDBRelation` between User and Profile, and
CommonDBRelation.php:634-641 requires **both** — upstream's own comment says so:

```php
// Always both checks for view
return static::canRelation('canView', true);
```

So granting `profile` alone leaves the Authorizations list empty with a 403
that, per Trap 3, names nothing.

## The tab/group taxonomy is in no API

`Profile::getRightsForForm()` (src/Profile.php:841) holds the structure the
profile form renders — `interface → tab → group → rights`:

```
central   tracking    itilobjects, tickets, followups_tasks, validations,
                      visibility, planning, problems, changes
          tools       general, projects
          assets      general
          management  general
          admin       general, inventory, rules, dictionaries
          setup       general
helpdesk  tracking | tools | setup    general
```

Those tab keys are the tabs in the UI, so the structure is what maps a right
onto where you click to change it. It is reachable by **no** supported route —
all four checked and verified:

| route | result |
|---|---|
| v2.3 | no grouping on any schema |
| v1 | `ProfileRight` gives names and values only |
| `GET /api.php/doc.json` (4.6 MB, 332 schemas) | 967 `x-itemtype`, **zero** `rightname` — cannot even link a right to an itemtype |
| `glpi-console` | no command touches rights (nothing in `src/Glpi/Console/` references `ProfileRight` or `getRightsForForm`) |

**A database account does not help.** The grouping is not in the database at
all: `glpi_profilerights` is the four columns above, and no other table holds a
right taxonomy. v1 already returns everything that table contains.

The only way to obtain it without transcribing it by hand is to call GLPI's own
function. That needs a real `$DB` — `getRightsForForm` reaches
`CommonDBTM::getTable()` and so `$DB->listFields()`, and a
`vendor/autoload.php`-only bootstrap dies with
`Call to a member function listFields() on null`. Hence `inc/includes.php`, and
hence the `glpi` user, who can read `config/`:

```sh
sudo -u glpi env GLPI_SRC=/nix/store/…-glpi-11.0.12 \
  GLPI_CONFIG_DIR=/var/lib/glpi/config \
  GLPI_VAR_DIR=/var/lib/glpi/files \
  GLPI_MARKETPLACE_DIR=/var/lib/glpi/marketplace \
  …/php dump-glpi-right-taxonomy.php
```

dumping `{right, interface, tab, group, label}` per right. Re-run on upgrade;
the output is GLPI's own authority rather than a reading of its source.

Any consumer of that dump should diff its key set against the live right names
from `ProfileRight` and show the difference, so an upgrade that moves a right
surfaces as an ungrouped entry rather than as a silently mis-filed one.

## Talking to v1 in practice

The sections above establish *that* auditing beyond the current session needs
v1. Three mechanics of actually doing it, none of them obvious:

**Trap 13 — v1 will not take the OAuth token.** The bearer that works for
every v2.3 call is rejected outright:

```
GET /api.php/v1/Profile_User   -> ["ERROR_SESSION_TOKEN_MISSING", ...]
GET /api.php/v1/initSession    -> ["ERROR_LOGIN_PARAMETERS_MISSING", ...]
```

Even `initSession` refuses it — it wants login/password or a user_token. So a
client speaking both APIs carries two authentication paths, not one: OAuth for
v2.3, and Basic auth exchanged for a session token for v1.

**Trap 14 — v1 sessions persist until something ends them.** `initSession`
allocates server-side state that lives on until it times out. A client that
takes one per request and never calls `killSession` accumulates them. Kill it
on every exit path, including the error path.

**Trap 15 — the rangeless default is far smaller than documented.** Measured
on 11.0.12, `GET /apirest.php/ProfileRight` with no range returns **15 of 899
rows**, not the 50 the documented default implies. The truncation is silent
unless `Content-Range` is read, so ranges belong on every v1 collection call
and a short answer should be refused rather than rendered.

Worth restating the cost, since the rest of this file is about v2.3: GLPI
describes v1 as *"closely tied to the GLPI source code … no promise of
stability between versions"*. It is used here because for these four
questions the alternative is not a worse endpoint, it is no answer. It does
**not** bypass permissions the way reading MariaDB would — every v1 call is
still made as the service account and still filtered by its profile.

## Applying a rights change

Rights are evaluated per request, so a change takes effect on the next call —
*but* a client caching its OAuth token keeps the session the old rights were
attached to. `glpi-inv` caches for up to an hour, so after editing a profile:

```sh
systemctl --user restart glpi-inv.service
```

## Minimum viable service account

Verified sufficient for `glpi-inv`:

- a profile with **Standard (central) interface**
- **Administration → Agent: Read** — the fleet
- **Administration → Entities: Read** — the scope line
- **Assets → Computer: Read** — the Computers page and its orphan check
- **Administration → Profiles: Read** — the Rights page, and
  `Profile_User`/`ProfileRight` through it
- **Administration → Users: Read** — required *together with* Profiles for the
  Authorizations list, per Trap 12
- authorization: **Root entity**, that profile, **Recursive ticked** → `(R)`
- the OAuth client needs the **`api` scope** ticked, or the token endpoint
  returns "You do not have the required scope(s)"
- for the v1 calls, `enable_api_login_credentials` — v1 wants Basic auth
  exchanged for a session token and rejects the OAuth bearer outright

Not needed: UPDATE or PURGE on any of them, nor any helpdesk right. This app
only ever reads.

This list was two rights when first written — `agent` and `entity`. It grew
with the app, and the only thing that catches such drift is that each page
states the right it needs; GLPI's 403 never will.

## Untested

- Whether a profile scoped to a *child* entity plus recursive behaves as
  expected — this instance has only the root entity, so the filtering path
  has never actually excluded a row here. The reasoning above is from the
  source, not from an observed exclusion.
- LDAP-granted (`D`) authorizations. None exist here.
- The taxonomy dump of the last section. The bootstrap requirement is verified
  (it fails without `$DB`, with the error quoted); the dump itself has not been
  run, because it needs the `glpi` user.
- Whether the module-defined bits of Trap 11 mean what their modules' own form
  definitions say. Only the leftover *values* are verified, never their labels.
- Every right name in the tables above is from this install. A plugin adds its
  own rights, and none are installed here.
