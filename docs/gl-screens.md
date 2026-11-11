# GLPI 12.0.1 — making and adjusting screens

How to create a screen that does not exist, and how to change one that does.
Written against the **12.0.1** source in
`/nix/store/…-glpi-12.0.1/`; every claim names
the file and line it came from so a later release can be checked against it
rather than trusted.

**Read the scope limit first.** Everything below is read out of the source: the
menu sectors from `getSectorizedDetails()`, the tabs from
`getTabNameForItem()`, the field lists from the arrays that build them, the
destructive behaviour from the methods that perform it. **The interface was not
driven.** Where this says "the Capacities tab", that tab is defined at
`AssetDefinition.php:128` — but no one here clicked it, and this file does not
describe where a button sits on screen. Treat it as a map of the mechanism, not
a click-path.

- **Five mechanisms make screens**, and they are not interchangeable
- **One mechanism adjusts an existing screen**, and it is a rights question
  more than a configuration one
- **The one irreversible action** is disabling a capacity — 27 of the 28
  force-delete the data they managed, with history off
- **The field list is the layout.** Order in `fields_display` *is* order on
  screen

---

## 0. Which mechanism you want

"Screen" covers five different things in GLPI. Picking the wrong one is the
expensive mistake, because four of them are not convertible into each other
afterwards.

| you want | mechanism | where it lives |
|---|---|---|
| a new **type of thing** to track, with its own list and form | **custom asset definition** | `src/Glpi/Asset/AssetDefinition.php` |
| a new **list of values** with its own management screen | **custom dropdown definition** | `src/Glpi/Dropdown/DropdownDefinition.php` |
| to **collect something from a person**, in steps | **Forms** | `src/Glpi/Form/` |
| a **read-only overview** of many things at once | **Dashboards** | `src/Glpi/Dashboard/` |
| to change what **end users land on** | **Helpdesk tiles** | `src/Glpi/Helpdesk/Tile/` |
| to change **which columns an existing list shows** | **`DisplayPreference`** | `src/DisplayPreference.php` |
| to change **which view you land on** for an existing list | **`SavedSearch`** | `src/SavedSearch.php:594` |

Menu sectors, from the source rather than from the menu:

| class | `getSectorizedDetails()` | reads as |
|---|---|---|
| `AssetDefinition` | `['config', self::class]` — line 70 | Setup → *(custom assets)* |
| `DropdownDefinition` | `['config', CommonDropdown::class, self::class]` — line 53 | Setup → Dropdowns → *(custom dropdowns)* |

`config` is GLPI's internal name for the Setup sector. Both definitions are
therefore **Setup-level objects**, not asset-level ones: making a screen is an
administrative act, and the right that governs it is a Setup right, not a
right on the assets the screen will hold.

---

## 1. Making a screen: the custom asset definition

This is the main route and the only one that produces a full
list-plus-form-plus-tabs screen for a thing you define yourself.

### 1.1 What a definition is, and what it is not

A definition creates a **class**, not a table. `AssetDefinition.php:100`:

```php
return sprintf('asset_%s', strtolower($this->fields['system_name']));
```

so a definition named `vehicle` yields a generated class keyed `asset_vehicle`.
Every custom asset of every definition then lives in **one shared table**,
`glpi_assets_assets`, told apart by `assets_assetdefinitions_id`.

**This is the single most misunderstood point about the mechanism**, and it is
worth being blunt because the wrong model leads to real planning errors:

- creating a definition **does not create a table**
- adding a custom field **does not add a column**
- so a second definition **does not buy a fresh column budget** — the common
  advice to split types "to get more columns" is reasoning about a structure
  that is not there

Split definitions for the reason that actually holds: each type then means one
thing, and each screen stays readable. Not for storage.

### 1.2 The four tabs a definition has

`getTabNameForItem()`, `AssetDefinition.php:113-156`. Exactly four, in this
order, each dispatched by `displayTabContentForItem()` at line 157:

| # | tab | icon | method | what it controls |
|---|---|---|---|---|
| 1 | **Capacities** | `ti ti-adjustments` | `showCapacitiesForm()` | which *sections* the screen has |
| 2 | **Fields** | `ti ti-forms` | `showFieldsForm()` | which *fields* the screen has, and in what order |
| 3 | **Profiles** | `ti ti-user-check` | `showProfilesForm()` | who can see and use it |
| 4 | **Translations** | `ti ti-language` | — | the type's name in each language |

Four JSON columns back them — `AssetDefinition.php:278`:

```php
foreach (['capacities', 'profiles', 'translations', 'fields_display'] as $json_field) {
```

So the entire shape of a custom screen is four JSON documents on one row. That
is why none of it needs a migration, and also why none of it is visible to a
SQL query that does not know to look inside JSON.

### 1.3 Tab 1 — Capacities: the 28 sections you can switch on

A capacity is a **whole section of the screen**, usually a tab, plus the
search options and relations behind it. `src/Glpi/Asset/Capacity/` holds 28
concrete classes (excluding `AbstractCapacity` and `CapacityInterface`).

Descriptions below are GLPI's own `getDescription()` strings, extracted from
each class rather than paraphrased:

| capacity | what enabling it adds |
|---|---|
| `IsInventoriable` | The GLPI agent can report inventory data for these assets |
| `HasDevices` | Includes sub-components like CPUs, drives or memory |
| `HasVolumes` | List storage volumes |
| `HasSoftwares` | List installed software |
| `HasOperatingSystem` | Display operating system information |
| `HasNetworkPort` | Has network ports (like ethernet and wlan) |
| `HasVirtualMachine` | List virtual machines attached to these assets |
| `HasAntivirus` | List antivirus software |
| `HasDatabaseInstance` | List database instances found by automatic inventory |
| `HasRemoteManagement` | Generate links for common remote access and control services |
| `HasInfocom` | Manage and track assets lifecycle, financial, administrative and warranty information |
| `HasContracts` | Link contracts to the assets for costs, renewal and supplier tracking |
| `HasCertificates` | Track certificates used by the assets |
| `HasDomains` | Track domains, records and their expiration dates |
| `HasDocuments` | Upload and attach files |
| `HasLinks` | Define associated external links for the assets |
| `HasKnowbase` | Knowledge base articles can be associated to these assets |
| `HasNotepad` | Enable a simple notepad |
| `HasHistory` | Records the modifications made to the asset |
| `HasImpact` | Enable "Impact analysis" tab and the assets can also be displayed in the impact tab of other asset types |
| `HasAppliances` | Can be part of an appliance. An appliance is a virtual object that groups several assets |
| `HasPeripheralAssets` | Can be connected to external peripherals or monitors |
| `HasSocket` | Manage sockets and cable links |
| `HasPlug` | Has power plugs. Usually related to PDU or UPS |
| `IsRackable` | Can be inserted in a datacenter rack |
| `IsReservable` | These assets can be made reservable |
| `IsProjectAsset` | Can be associated to a project |
| `AllowedInGlobalSearch` | Include in global search (from the header bar) results |

Two of these are worth calling out when designing a screen:

- **`IsInventoriable`** is what connects a custom type to the agent. Without
  it, a definition is a manual register; with it, inventory can populate it.
- **`AllowedInGlobalSearch`** is the difference between a type people can find
  and one they must navigate to. Cheap, and easy to forget.

### 1.4 ⚠ Disabling a capacity destroys its data

**This is the one action in this document that cannot be undone, and nothing
about the word "capacity" suggests it.**

`AssetDefinition.php:405-428` diffs old against new capacities on save and
calls `onCapacityDisabled()` for each one removed.

`AbstractCapacity::onCapacityDisabled()` is an **empty default** (line 193), so
this is decided per capacity — and **27 of the 28 override it and delete**.
The sole exception is `AllowedInGlobalSearch`, which overrides it with no
deletion at all, because it owns no data: it only toggles whether the type
appears in header-bar search. Every other capacity on the §1.3 list destroys
what it managed.

Taking `HasVolumesCapacity.php:95-109` as the pattern:

```php
$disks->deleteByCriteria(['itemtype' => $classname], force: true, history: false);
$this->deleteRelationLogs($classname, Item_Disk::class);
$this->deleteDisplayPreferences($classname, $disks_search_options);
```

Read those three arguments:

- **`force: true`** — a purge, not a move to trash. There is nothing to
  restore from.
- **`history: false`** — the deletion is not written to the history either, so
  afterwards there is no record that the data existed.
- and the **display preferences** for those search options are deleted too,
  so any column layout built on them is gone as well.

So toggling a capacity off to "tidy up the screen" deletes every volume, or
every software entry, or every network port, on every asset of that type —
silently and permanently. The same happens on purge of the whole definition
(`cleanDBonPurge()`, line 436).

`HasSoftwares` and `IsInventoriable` carry the most deletion calls of the
twenty-seven (seven and five respectively), which is a rough proxy for how
much they tear out.

**Before disabling any capacity:** take a database dump. The manual's backup
step is `mariadb-dump` — see `gl-client-serv.md` §5. A capacity you merely
want hidden is a `fields_display` question (§1.5), not a capacity question.

### 1.5 Tab 2 — Fields: the list *is* the layout

`showFieldsForm()` renders `pages/admin/assetdefinition/fields_display.html.twig`
(line 211). What it writes, line 340-350:

```php
foreach ($input['fields_display'] as $field_order => $field_key) {
    $formatted_fields_display[] = [ 'key' => $field_key, 'order' => $field_order, ... ];
}
```

The array **index is the order**. There is no separate layout model, no rows
or columns to place: the sequence of the list is the sequence on the form.
Reordering the list reorders the screen, and removing an entry removes the
field from the form without touching any data.

That last point is the counterpart to §1.4 and the reason to be precise about
which you want:

| you want | do this | data |
|---|---|---|
| field off this screen | remove it from **`fields_display`** | **kept** |
| whole section off this screen | disable the **capacity** | **destroyed** |

**The 17 core fields available**, in GLPI's own default order
(`getAllFields()`, line 710):

| # | key | label | type |
|---|---|---|---|
| 1 | `name` | Name | `StringType` |
| 2 | `template_name` | Template name | `StringType` |
| 3 | `states_id` | Status | `DropdownType` |
| 4 | `_dc_breadcrumbs` | Data center position | `RawType` |
| 5 | `locations_id` | Location | `DropdownType` |
| 6 | `users_id_tech` | Technician in charge | `DropdownType` |
| 7 | `manufacturers_id` | Manufacturer | `DropdownType` |
| 8 | `groups_id_tech` | Group in charge | `DropdownType` |
| 9 | `contact_num` | Alternate username number | `StringType` |
| 10 | `serial` | Serial | `StringType` |
| 11 | `contact` | Alternate username | `StringType` |
| 12 | `otherserial` | Inventory number | `StringType` |
| 13 | `users_id` | User | `DropdownType` |
| 14 | `groups_id` | Group | `DropdownType` |
| 15 | `uuid` | UUID | `StringType` |
| 16 | `comment` | Comments | `TextType` |
| 17 | `autoupdatesystems_id` | Auto-update system | `DropdownType` |

Plus **two dynamically keyed** dropdowns the table above cannot name
statically — the definition's own *type* and *model* dropdowns, labelled from
`$type_class::getFieldLabel()` and `$model_class::getFieldLabel()`. And then
every custom field, keyed `custom_<system_name>` (line 795).

If `fields_display` is empty or unparseable, `getDefaultFieldsDisplay()` (line
805) supplies the full list in the order above — so a definition starts with
everything shown and is narrowed from there, rather than built up from nothing.

### 1.6 Tab 3 — Profiles

The `profiles` JSON controls who can see and use the type. One entry worth
knowing is separate from plain read/write: line 935 defines

> *Profiles that can associate %s with tickets, problems or changes*

so "can this asset be attached to a ticket" is its own per-profile switch, not
implied by being able to read the asset. A type that looks invisible to a
technician is as likely to be a profile gap as a capacity gap — and
`gl-rights.md` in this directory is the companion for diagnosing that, since a
403 in GLPI tells you nothing about which right was missing.

### 1.7 Tab 4 — Translations

The type's name per language, held in the `translations` JSON. Worth setting
even in a single-language deployment: the plural form is what list headers and
tab labels use, and an unset plural shows up as awkward text in places you did
not expect to look.

---

## 2. Custom fields in detail

A custom field is a **row**, and its values live in **one JSON column**.

### 2.1 Storage

- definitions: rows in `glpi_assets_customfielddefinitions`, each belonging to
  one asset definition (`CustomFieldDefinition.php:56`,
  `$itemtype = AssetDefinition::class`)
- values: a single `custom_fields json` column on `glpi_assets_assets`, keyed
  by definition id — `json_encode` on save, `json_decode` on read
  (`Asset.php:517` and `:526`)

So fifty custom fields and one custom field occupy the same one column. What
bounds the count is therefore not a column budget: MariaDB implements `json`
as `longtext`, so the ceiling is a server setting, and long before that the
screen stops being readable. A working figure of **15 to 30 fields** per type
is a convention for legibility, not a measured threshold — treat it as the
point to stop and reorganise.

### 2.2 The eight field types you can pick

`src/Glpi/Asset/CustomFieldType/`, excluding `AbstractType` and
`TypeInterface`, holds nine concrete classes — but the manager's scan keeps a
class only if its static `isAllowedForCustomFields` returns true, and `RawType`
returns false (`RawType.php:46`). Counting the directory gives nine; counting
what the picker offers gives eight.

| type | use for |
|---|---|
| `StringType` | one line of text |
| `TextType` | multi-line text |
| `NumberType` | numeric values |
| `BooleanType` | yes/no |
| `DateType` | a date |
| `DateTimeType` | a date and time |
| `DropdownType` | a value from an existing GLPI itemtype |
| `URLType` | a link |
| `RawType` | **not offered** — core-only; `_dc_breadcrumbs` (field 4 above) is one |

`DropdownType` is the one to reach for more often than feels natural. It takes
`allowed_dropdown_itemtypes` (`CustomFieldDefinition.php:150`), so it points at
real GLPI objects rather than free text — which means the value can be
validated, reported on, and renamed in one place. Twenty text fields that
nothing can validate is the failure this prevents.

### 2.3 The four field options

`src/Glpi/Asset/CustomFieldOption/`: `BooleanOption`, `NumberOption`,
`ProfileRestrictOption`, behind `OptionInterface`. Stored as the
`field_options` JSON on the definition row (line 217).

**`ProfileRestrictOption` is a screen control, not a data control.** It is how
one field is shown to some profiles and not others — the finest-grained
screen adjustment the mechanism offers, and the right tool when the answer to
"should this be on the screen" is "depends who is looking".

### 2.4 The naming rule

`CustomFieldDefinition.php:172`:

```php
if (!is_string($input['system_name']) || preg_match('/^[a-z0-9_]+$/', $input['system_name']) !== 1)
```

**Lowercase letters, digits and underscores only.** No hyphens, no capitals,
no spaces, no accents. The `label` is the human-facing name and is free; the
`system_name` is the key the JSON is written under, which is why it is
constrained — and why **renaming it later is not a rename but a new field**,
leaving the old values keyed under a name nothing reads. Choose it once.

---

## 3. Adjusting an existing screen

Nothing above applies to GLPI's built-in types. A Computer's form is not a
definition and has no `fields_display`. What is adjustable there is narrower
and mostly about **lists**.

### 3.1 Which columns a list shows — `DisplayPreference`

`src/DisplayPreference.php`. Right: **`search_config`**, with two scopes
(lines 55-56):

```php
public const PERSONAL = 1024;
public const GENERAL  = 2048;
```

The same mechanism serves both "my columns" and "the default for everybody",
separated only by which bit of `search_config` you hold —
`showFormPerso()` at line 607, `showFormGlobal()` at line 649, each gated on
its own constant (lines 186 and 219).

**So "can I change this list for everyone?" is a rights question, not a
feature question.** With `PERSONAL` only, every change you make is yours
alone; the person asking why their colleagues do not see it needs `GENERAL`,
not a different screen. This is the most common adjustment people want and the
most commonly misdiagnosed.

Note the link back to §1.4: `deleteDisplayPreferences()` is part of disabling
a capacity, so a global column layout can be destroyed as collateral damage
from a capacity toggle on a custom type.

### 3.2 Which view you land on — `SavedSearch`

`SavedSearch.php:594` `markDefault()`, writing `glpi_savedsearches_users`
(line 606); `unmarkDefault()` at 637 and `unmarkDefaults()` at 672. Right:
`bookmark_public` for the shared ones.

A saved search carries its own filters *and* its column selection, and marking
one default changes what you see on arrival. For "this list should open
already filtered", this is the mechanism — not `DisplayPreference`, which only
decides the columns. The default is held **per user**
(`glpi_savedsearches_users`), so it is a personal landing choice built on a
possibly shared search.

### 3.3 What appears at all — Profiles

Covered by `gl-rights.md`. The two fields most relevant to screens are
`helpdesk_hardware` and `helpdesk_item_type` on the Profile schema, which
govern what a simplified-interface user is offered. Worth stating because an
"empty screen" bug is frequently a profile, and GLPI's 403 names no right.

---

## 4. The other four screen makers

Not elaborated to the depth above — each is a subsystem — but placed so the
choice in §0 can be made knowingly.

### Forms — `src/Glpi/Form/`

`Form.php` plus `QuestionType/`, `Condition/`, `Destination/`,
`AccessControl/`, `ServiceCatalog/`, `Translation`. A form is an **intake**
screen: it asks a person questions, optionally branching on their answers
(`Condition/`), and turns the result into something (`Destination/`) such as a
ticket. `AccessControl/` decides who may open it; `ServiceCatalog/` is how it
is published for end users.

Use a form when you are collecting a lot **from a person**, and an asset
definition when you are recording a lot **about a thing**. Answers are kept
relationally (`Answer.php`, `AnswersSet.php`), which is a different shape from
an asset's flat field list and the right one for a request.

### Dashboards — `src/Glpi/Dashboard/`

`Dashboard.php`, `Grid.php`, `Widget.php`, `Provider.php`, `Filter.php`,
`Palette.php`, and `Right.php` for per-dashboard access
(`$items_id = 'dashboards_dashboards_id'`). A dashboard is a grid of widgets
fed by providers — a **read-only overview of many things**, not a place to
enter or edit one thing.

### Helpdesk tiles — `src/Glpi/Helpdesk/Tile/`

`GlpiPageTile`, `FormTile`, `ExternalPageTile`, assembled by `TilesManager.php`
with `Item_Tile` as the join, plus `HomePageTabs.php`. This composes the
**landing screen end users see**. The three tile classes are the whole
vocabulary: point at a GLPI page, point at a form, or point outside GLPI.

### Custom dropdown definitions — `src/Glpi/Dropdown/DropdownDefinition.php`

The same definition pattern as assets, scoped to a list of values, and
reachable at Setup → Dropdowns per its `getSectorizedDetails()`. Reach for it
when the thing you are modelling is a **vocabulary** rather than an asset —
and prefer a nested dropdown over spreading a hierarchy across several text
fields, since the structure then lives in one validated place.

---

## 5. Choosing where to put data

The order worth trying, given everything above:

1. **A dedicated asset definition**, when you are tracking a distinct kind of
   thing. Fifty fields on Computer is the smell; a `vehicle` type with its own
   screen is the fix. The gain is that each type means one thing — **not**
   extra storage, since every custom asset shares `glpi_assets_assets`.
2. **A form**, when the data comes from a person as a request.
3. **A dropdown tree**, when the data is a structured vocabulary.
4. **Custom fields on an existing definition**, for the genuinely
   type-specific remainder — and `DropdownType` rather than `StringType`
   wherever the value is really a reference to something.

Tickets have **no native custom fields**: this is an asset-definition feature.
Extra data on a ticket comes from Forms or a plugin, so none of §1 or §2
transfers to the ticket screen.

---

## Traps

**1. Disabling a capacity purges its data, unlogged.** `force: true,
history: false`, for **27 of the 28** — all but `AllowedInGlobalSearch`. The
word "capacity" sounds like visibility. It is not. §1.4.

**2. Removing a field from `fields_display` does not delete anything.** The
safe way to simplify a screen. Reach for this before reaching for capacities.

**3. A second definition does not give you more columns.** There are no
per-field columns and no per-type table. Splitting types is an organisational
decision with organisational benefits.

**4. `system_name` cannot be renamed meaningfully.** `/^[a-z0-9_]+$/`, and it
is the JSON key the values are stored under. A "rename" orphans the old data.

**5. `DisplayPreference` changes may be yours alone.** `PERSONAL` vs
`GENERAL` are two bits of one right. Nothing in the interface will explain why
a colleague does not see your columns.

**6. A capacity toggle can delete a global column layout.**
`deleteDisplayPreferences()` runs inside `onCapacityDisabled()`, so the
collateral damage in trap 1 extends past the asset data itself.

**7. An empty or unreadable `fields_display` means "show everything", not
"show nothing".** `getDefaultFieldsDisplay()`, line 805. A definition that
suddenly shows every field has probably had that JSON invalidated rather than
reset.

**8. The four tabs are the whole configuration surface.** Capacities, Fields,
Profiles, Translations. If what you want is not one of those four, it is not an
asset-definition adjustment and §0 is the table to re-read.

---

## Not verified here

- **No part of the interface was driven.** Menu sectors, tabs, field lists and
  destructive behaviour were all read in the source. Button positions, dialog
  wording and whether any confirmation is shown before a capacity is disabled
  are **unknown** — and given trap 1, whether GLPI warns first is the single
  thing most worth checking by hand before trusting this file in anger.
- **No custom asset definition exists on this install.** Nothing here was
  exercised against live data; there is no custom type on `nixos.local` to
  exercise it against.
- **The 15-to-30 field figure is a convention**, not a measurement. Nothing
  here benchmarked a GLPI install with hundreds of custom fields.
- **Forms, Dashboards and Helpdesk tiles are placed, not documented.** §4 says
  what each subsystem is for and names its classes; none of the three was read
  to the depth of §1.

## Sources

| claim | file |
|---|---|
| menu sector, tabs, JSON fields, capacity diffing, field list | `src/Glpi/Asset/AssetDefinition.php` |
| capacity behaviour on disable | each concrete `src/Glpi/Asset/Capacity/*.php` — the base `AbstractCapacity::onCapacityDisabled()` (line 193) is an empty default; it supplies only the `deleteRelationLogs()` (206) and `deleteDisplayPreferences()` (267) helpers |
| the 28 capacities and their descriptions | `src/Glpi/Asset/Capacity/*.php` |
| field types, options, naming rule | `src/Glpi/Asset/CustomFieldDefinition.php`, `CustomFieldType/`, `CustomFieldOption/` |
| value encoding | `src/Glpi/Asset/Asset.php:517`, `:526` |
| column scopes and rights | `src/DisplayPreference.php` |
| default view | `src/SavedSearch.php:594` |
| dropdown definitions | `src/Glpi/Dropdown/DropdownDefinition.php` |

## See also

| file | what |
|---|---|
| `gl-client-serv.md` | the server/client setup manual, including the backup to take before §1.4 |
| `gl-rights.md` | profile rights, and why a GLPI 403 names nothing |
| `gl-db-api.md` | why custom assets and custom dropdowns are the one API family that *does* come from the database |
| `gl-upgrade.md` | what a GLPI upgrade breaks, and which failures are silent |
