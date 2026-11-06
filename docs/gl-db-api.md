# GLPI 11.0.12 — what defines the API: code or database?

Whether GLPI's API surface lives in its source or in its database, and which
of the two you have to change to affect it. Everything marked **verified** was
either read in the GLPI 11.0.12 source in `/nix/store/…-glpi-11.0.12/` or
observed against the live instance on `localhost:8080`; nothing here is
inferred from documentation.

Written while deciding whether a read-only MariaDB account would help
`glpi-inv` reach things its API would not give up. It would not — which is the
short answer this file exists to record.

- **Are the endpoints defined in the database?** No — PHP attributes — verified
- **Are the field lists (schemas)?** No — PHP literals — verified
- **Is anything about the API in the database?** Yes: whether it is enabled,
  and who may authenticate — verified
- **Any endpoints that genuinely come from the database?** Yes, one family:
  custom assets and custom dropdowns — verified

---

## The endpoints are code

Every route is a PHP attribute on a controller method:

```php
#[Route(path: '/Profile', methods: ['GET'], middlewares: [ResultFormatterMiddleware::class])]
public function searchProfiles(Request $request): Response
```

**539 of them** across the high-level controllers, counted with
`grep -c '#\[Route('` over `src/Glpi/Api/HL/Controller/`:

| controller | routes | | controller | routes |
|---|---|---|---|---|
| AssetController | 138 | | DropdownController | 17 |
| AdministrationController | 61 | | CustomAssetController | 17 |
| ITILController | 59 | | CoreController | 16 |
| ProjectController | 43 | | ComponentController | 14 |
| ManagementController | 43 | | InventoryController | 9 |
| SetupController | 29 | | ReportController | 8 |
| NotificationController | 28 | | ToolController | 7 |
| RuleController | 21 | | NotepadController | 5 |
| KnowbaseController | 20 | | GraphQLController | 3 |
| | | | DashboardController | 1 |

The schemas behind them — the field list each endpoint returns, with its
`x-itemtype` — are PHP array literals in those same controllers. The `Profile`
schema at AdministrationController.php:645-699 is a worked example: eight
properties, written out by hand.

Consequence: nothing in the database decides that `/Inventory/Agent` exists, or
which fields it returns. **Changing the API means changing PHP.**

This is also why the profile-form right taxonomy is unreachable from the
database — see gl-rights.md. It is code, so no SQL account can produce it.

## What the database does control

The database governs whether the door opens and who gets through it, not where
the doors are.

| table | what it decides |
|---|---|
| `glpi_configs` | whether the API runs at all, and where it advertises itself |
| `glpi_oauthclients` | who may authenticate to v2.x, and with which scopes |
| `glpi_oauth_access_tokens`, `glpi_oauth_refresh_tokens`, `glpi_oauth_auth_codes` | live OAuth2 token state |
| `glpi_apiclients` | the legacy v1 App-Token clients |
| `glpi_profilerights` | what an authenticated caller may *see* through any endpoint |

### The config flags

Read from `$CFG_GLPI`, which is loaded from `glpi_configs`:

- `enable_api` — Config.php:624
- `enable_hlapi` — Config.php:2144, `(bool) ($CFG_GLPI['enable_hlapi'] ?? 0)`
- `enable_api_login_credentials` — required for the v1 Basic-auth sign-in
- `url_base_api` — the address GLPI advertises for its own endpoints, which is
  wrong by default on a non-80 port; see docs/oauth-setup.md in glpi-inv

All four are set with `glpi-console config:set`, not by editing a file.

### `glpi_apiclients` is more than a token list

```
app_token, app_token_date
ipv4_range_start, ipv4_range_end, ipv6     -- source IP filtering
is_active
entities_id, is_recursive                  -- entity scope
dolog_method
```

So the legacy API's access control — token, source address, entity scope — is
entirely database state. The high-level API has no equivalent table: its access
control is the OAuth client plus the user's own profile.

## The exception: custom assets come from the database

`CustomAssetController` is the one place where the surface is built from rows
rather than written out. CustomAssetController.php:78-95:

```php
$definitions = AssetDefinitionManager::getInstance()->getDefinitions();

foreach ($definitions as $definition) {
    $asset_class = $definition->getAssetClassName();
    $asset_system_name = $definition->fields['system_name'];
    $schema_name = 'CustomAsset_' . $asset_system_name;
```

One schema per row in `glpi_assets_assetdefinitions`, **named after the row**.
`glpi_dropdowns_dropdowndefinitions` does the same for custom dropdowns.

The route template is still code, but what resolves against it is data:

```php
#[Route(path: '/{itemtype}', methods: ['GET'],
        requirements: ['itemtype' => [self::class, 'getCustomAssetTypes']], …)]
```

`getCustomAssetTypes` is a callback reading those same definitions, so the set
of valid `{itemtype}` values is whatever the database holds.

**Verified live on this install**, which has no custom assets defined:

```
GET /api.php/doc.json          1219 paths, 332 schemas
  paths under /Assets/Custom   7   (the templates)
  CustomAsset_* schemas        0   (nothing to fill them)
GET /api.php/v2.3/Assets/Custom  →  200  []
```

Define a custom asset type in GLPI's UI and endpoints for it appear with no
code change. So `doc.json` is not a fixed surface: it grows with what is
defined in the database. Any client that treats it as a constant is wrong in
principle, though nothing `glpi-inv` calls is affected — it touches only fixed
endpoints.

## The rights on an endpoint: name in code, values in the database

Two different questions, with two different answers.

**Which right an endpoint requires is code.** `ResourceAccessor.php:353`
authorises a collection with `$itemtype::canView()`, and that resolves
`static::$rightname` — a static PHP property. Delegating types are code too:
`ProfileRight` declares none of its own, so `CommonDBChild::canView`
(CommonDBChild.php:130-136) falls through to its parent and it needs `profile`;
`Profile_User` is a relation and `CommonDBRelation::canView`
(CommonDBRelation.php:634-641) requires **both** `user` and `profile`, under
upstream's own comment `// Always both checks for view`.

**The values are database, entirely.** `glpi_profilerights` — 899 rows on this
install, four columns, nothing else. See gl-rights.md.

### A correction

An earlier version of this file, and of glpi-inv's dump script, said that five
classes "compute their `$rightname` at runtime". **That is false.** Every one
is a static literal; a naive single-quote regex over the class's own file just
cannot see them:

| class | where its right actually comes from |
|---|---|
| `self::class` in Profile.php | the enclosing class — `Profile`, so `profile` |
| `Certificate` | `$rightname = "certificate"` — **double** quotes |
| `TicketTask` | inherits `CommonITILTask::$rightname = 'task'` |
| `TicketTemplate` | inherits `ITILTemplate::$rightname = 'itiltemplate'` |
| `Item_DeviceSimcard` | inherits `Item_Devices::$rightname = 'device'` |
| `Grid` | not an itemtype right at all |

A parser handling both quote styles, `self::class` and one level of inheritance
would resolve all of them. So the case for dumping the taxonomy from GLPI is not
that parsing is impossible — it is that `getRightsForForm()` is the function
that renders the form, so its answer cannot drift from what the form shows,
where a regex walking PHP inheritance chains across upgrades can.

### The exception: custom objects define their right in the database

Custom assets and custom dropdowns break the "name in code" rule outright. The
generated class is PHP *source built at runtime*, with the right name
interpolated into it — AssetDefinitionManager.php:409-420:

```php
$rightname = $definition->getCustomObjectRightname();
...
public static \$rightname = '{$rightname}';
```

and that name is derived straight from the definition row:

```php
// AssetDefinition.php:98-101
return sprintf('asset_%s', strtolower($this->fields['system_name']));
// DropdownDefinition.php:93-96
return sprintf('dropdown_%s', strtolower($this->fields['system_name']));
```

GLPI then keeps `glpi_profilerights` in step with it:
`AbstractDefinition.php:695-719` synchronises the definition's `profiles` field
through `ProfileRight::updateProfileRights` and `ProfileRight::deleteProfileRights`.

So defining a custom asset named `Kiosk` creates the endpoint, the right
`asset_kiosk`, and its profilerights rows — with no code change. For that
family, the right on the endpoint **is** defined in the database.

### …and two of them are on no profile form, ever

A right created this way is granted per profile and enforced like any other.
Whether it appears on the form that `getRightsForForm()` describes is a
separate question, and for two kinds the answer is never. Profile.php:1187,
inside that function:

```php
// Add rights for custom assets
$definitions = AssetDefinitionManager::getInstance()->getDefinitions(only_active: true);
foreach ($definitions as $definition) {
    $all_rights['central']['assets']['custom_assets'][] = ...
}
```

- **Active custom assets** land under `central → assets → custom_assets`, a
  group that exists only once one is defined. Re-running the dump picks them up.
- **Inactive custom assets** are excluded by `only_active: true`. The right
  exists in `glpi_profilerights`; the form never mentions it.
- **Custom dropdowns** are not added to the form at all. The only
  `DropdownDefinitionManager` loop in Profile.php is at line 348, inside
  `cleanDBonPurge()` — profile deletion, not the form. Checked by resolving
  which function each of the six definition call sites belongs to, because
  line 348 reads like form code from its surroundings and is not.

So a consumer grouping rights by that taxonomy has a permanently unplaceable
remainder, and "refresh the map" is not the fix for it. `glpi-inv` files those
under *Not on the profile form*, which is accurate for both reasons at once —
stale copy, and never-listed.

That block also sits *after* the `$GLPI_CACHE->set()` call, so the custom part
is recomputed on every call rather than cached. A dump therefore reflects the
definitions active at the moment it runs.

Nothing here has any custom objects defined, so this is mechanism read from the
source, not an observed example — see Untested.

## So: would a database account help?

For reaching API-invisible data, sometimes. For changing or discovering the API
surface, no — that is PHP, with the single custom-asset exception above, and
that exception is driven through GLPI's own UI rather than by hand-editing
rows.

Specifically **not** obtainable from the database:

- the endpoint list, or any endpoint's field list
- the right → (tab, group) taxonomy of the profile form (gl-rights.md)
- which right an endpoint requires — that is `$itemtype::canView()` resolving
  `static::$rightname`, a static PHP property

And the cost of taking one is real: direct SQL bypasses the entity filtering
every API path applies, and couples the client to GLPI's schema across
upgrades. Using the legacy v1 API instead does neither — same account, same
profile rights, same entity scope.

## Untested

- Writing through any of this. Everything above was read; nothing was created
  or modified through either API.
- Custom assets with an actual definition present. This install has none, so
  neither the generated schemas nor the generated `asset_<name>` rights have
  ever been observed populated — the mechanism is from the source plus the
  empty live result, not from a working example. In particular the claim that
  creating one seeds its `glpi_profilerights` rows is read from
  AbstractDefinition.php, never watched happen.
- `glpi_apiclients` IP filtering in effect. The columns are verified from the
  schema; no client has been registered here to exercise them.
- Plugins. A plugin can register its own routes, and none are installed.
