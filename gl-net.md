# GLPI on NixOS — loopback install, client-server over TLS, and what runs now

Working notes from putting GLPI on this machine (NixOS 26.05) as a
local-only dashboard first, then opening it to the LAN with a self-signed
certificate and inventory agents. Part 3 records the deployed state as it
stands. Everything marked **verified** was executed here and its output read;
everything marked **untested** was not, and says so.

Built on **11.0.12** and upgraded to **12.0.1** on 2026-11-04. The notes were
written against 11.0.12, so where one names a version, that is the version it
was checked against — not a claim it was re-verified afterwards.

- **GLPI 12.0.1, loopback only:** working — login and dashboard verified
  (on 11.0.12, before the upgrade)
- **PHP 8.4.27 from a second nixpkgs input:** working — two live RCE CVEs closed
- **LAN over HTTPS + mDNS + inventory agent:** live — `0.0.0.0:443` up, the
  agent reporting daily, TLS verified by the client
- **Windows / macOS clients:** written from upstream docs, **untested here**

nixpkgs has **no `glpi` package and no `services.glpi` module** — checked with
`builtins.hasAttr` against this machine's own `pkgs` and `options`, not guessed
from `nix search` suggestions. Both are hand-written in `/etc/nixos/glpi.nix`.
It does have `pkgs.glpi-agent` (1.20) but **no `services.glpi-agent`** — only
`services.fusionInventory`, which drives the abandoned predecessor. So the
agent module is hand-written too, in `/etc/nixos/glpi-agent.nix`.

---

## Part 1 — the loopback install

```sh
# the whole stack
sudo nixos-rebuild switch --flake /etc/nixos#nixos --max-jobs 2 --cores 2

# one-time schema creation (NOT done by a rebuild, deliberately)
sudo -u glpi glpi-console database:install \
  --db-name=glpi --db-user=glpi --db-password= --no-interaction

# routine administration
sudo -u glpi glpi-console <command>
```

Stack: GLPI 12.0.1 release tarball → nginx → php-fpm (PHP 8.4.27) → MariaDB
11.4.12 over a unix socket. Source read-only in `/nix/store`, all state in
`/var/lib/glpi`.

### The design hinges on one upstream detail

GLPI normally writes **inside its own source tree**, which the store forbids.
`src/Glpi/Application/SystemConfigurator.php:190` defines every path constant
like this:

```php
if (!defined($name) && ($value = getenv($name)) !== false) define($name, $value);
```

So all of them can be redirected by environment variable. Setting
`GLPI_VAR_DIR` covers most: `_cache`, `_log`, `_sessions`, `_tmp`, `_lock`,
`_graphs` and the rest default to `{GLPI_VAR_DIR}/...` and GLPI creates them
itself (line 282). Only `GLPI_CONFIG_DIR`, `GLPI_VAR_DIR` and
`GLPI_MARKETPLACE_DIR` need naming.

**Verified, not assumed:** ran `bin/console` from the read-only store path with
those three vars pointed at a scratch directory. GLPI created all 14 state
subdirectories there and never touched the store.

The same three vars go to php-fpm, the cron unit *and* the `glpi-console`
wrapper. All three must agree — a console run with different paths writes a
`config_db.php` the web process cannot see.

### Why PHP comes from a second nixpkgs input

The system pin (`7993f34`, Oct 12) carries **php 8.4.26**, which is vulnerable
to:

| CVE | CVSS | note |
|---|---|---|
| CVE-2026-109618 | 9.8 | RCE via uninitialised request memory after an OOM `longjmp` |
| CVE-2026-109619 | 9.2 | use-after-free RCE; **CISA KEV**, deadline 2026-10-29 |

Both are **interpreter** bugs. No GLPI version avoids them — GLPI 11.0.12 and
12.0.1 shipped the same eight application fixes on Oct 28 and both run on
whatever PHP they are given. Only a newer PHP helps.

Moving the whole `nixpkgs` input forward *would* fix it (branch head already
had 8.4.27) but forces a from-source rebuild of `claude-code`, which is **not
in cache.nixos.org**, and that **OOM-killed the nix-daemon** on this 7.5 GB box
(3.6 GB RSS, global OOM, Chrome's thread pool as the trigger). So `flake.nix`
grew a second input used for PHP alone:

```nix
nixpkgs-php.url = "github:NixOS/nixpkgs/08d3a97586f6b6343b3b5e799980773629432610";
```

reaching the module via `specialArgs = { inherit inputs; }`, and in `glpi.nix`:

```nix
phpPkgs = inputs.nixpkgs-php.legacyPackages.${pkgs.stdenv.hostPlatform.system};
php = phpPkgs.php84.buildEnv { ... };
```

It deliberately does **not** `follows` nixpkgs — that would pull PHP back to
8.4.26 and defeat the point. Result: 55 derivations and 74 MiB instead of a
world rebuild. nginx and MariaDB still come from the main pin; the two
revisions never interact.

**This is temporary.** When the main pin next carries PHP ≥ 8.4.27, delete the
input and the `phpPkgs` line. `pkgs.php84` in the system is still 8.4.26 — it
is never built because nothing references it (0 matching store paths), but a
*second* PHP app written against `pkgs.php84` would build the vulnerable one.

GLPI's own checker agrees the result is sound: `system:check_requirements` →
**23 OK, 0 errors, 0 warnings**, including `[SECURITY] PHP maintained version
[OK]`.

---

## Part 2 — client-server

The reasoning and the traps are below; what is actually deployed right now —
listeners, certificate, agent config, observed timings — is Part 3.

```nix
services.glpi.lan.enable = true;          # HTTPS + firewall
services.glpi-agent = {                   # this box inventories itself
  enable = true;
  server = "https://${config.services.glpi.hostname}/";
  caCertFile = "${config.services.glpi.lan.tlsDir}/cert.pem";
};
```

Written against the options rather than against literal values, because that
is what `configuration.nix` actually contains — a hostname or path copied in
here would be a second place to update and a second place to be wrong.

Loopback HTTP on 8080 is **kept** — one vhost, two listeners, so the routing
rules cannot drift between a local and a LAN copy.

### The certificate is generated at runtime, never in a derivation

Anything Nix builds lands in `/nix/store`, which is **world-readable**. Fine
for a certificate, fatal for a private key. So a oneshot unit
(`glpi-cert.service`, ordered `before = nginx.service`) creates both under
`/var/lib/glpi-tls` on first boot and leaves them alone afterwards —
regenerating would invalidate the copy already distributed to clients.

**SAN is not optional**: browsers and the agent's OpenSSL both ignore CN
entirely, so a certificate carrying only a CN fails verification no matter how
correct the name is. Subject, SAN and expiry as issued: Part 3.

### Agents verify properly — do not use `--no-ssl-check`

Agent 1.20 has `--ca-cert-file` and `--ssl-fingerprint`, so a self-signed
server gets both **encryption and authentication**. `--no-ssl-check` accepts
any certificate: encryption against a passive listener, nothing against an
active one.

That distinction matters here specifically. This box is on **WPA2 with
`wpa-psk`** (not WPA3/SAE), PMF at default. WPA2-PSK gives **no isolation
between devices that know the passphrase** — per-session keys derive from the
shared PSK plus the 4-way handshake, so any device with the WiFi password can
capture a peer's handshake and decrypt that peer's traffic, and with PMF off
forcing a re-handshake is trivial. Traffic also crosses the router's wired side
in clear. "My WiFi is encrypted" is therefore not a substitute for TLS unless
nobody else has the passphrase.

What was at risk over plain HTTP was not really the inventory data: the browser
POSTs `login_password=` to `/front/login.php`, and the session cookie is a
128-char `glpi_<hash>` whose replay is a full admin session.

Config keys read from the agent's own parser (`GLPI/Agent/Config.pm`): file
format is `key = value`, keys match the long options. `ca-cert-file` and
`ca-cert-dir` are **mutually exclusive** — both set is a fatal error (line 332).

### mDNS is load-bearing

`services.avahi` already had `enable`, `nssmdns4`, `openFirewall` for the
network printer. That **resolves other hosts' `.local` names and never
announces its own** — `getent hosts nixos.local` returned nothing. Publishing
is a separate switch:

```nix
services.avahi.publish = { enable = true; addresses = true; workstation = true; };
```

The certificate is bound to the name, so a client that cannot resolve it needs
a hosts entry — or the IP, which is **not in the SAN**, and using it means
reissuing. The address is a DHCP lease (NetworkManager, no static config),
which is precisely why the name was chosen over it; pin it in the router
anyway. Current address: Part 3.

### Native inventory ships DISABLED

`/Inventory` and `/front/inventory.php` both return **403** until it is turned
on — `InventoryController.php:66` throws `AccessDeniedHttpException` unless
`enabled_inventory == 1`. Reproducible rather than a UI click:

```sh
sudo -u glpi glpi-console config:set --context=inventory enabled_inventory 1
```

Context `inventory` comes from `Conf.php`'s
`Config::getConfigurationValues('inventory')`.

Both endpoints are **routes through `public/index.php`**, not files — see trap 2.

### Agent runs as root, with tools on PATH

An inventory agent reads DMI/SMBIOS tables, block device serials and the
package database. Unprivileged it starts, **succeeds, and reports a near-empty
machine** — the failure mode is silent, which is why this is worth stating at
all. The same is true of its helper tools: a missing one yields missing fields,
not an error, so they are put on the unit's `path` deliberately. Which tools,
and the unit as it currently runs: Part 3.

### What "inventory" actually collects — and two categories turned off

"Inventory" here means automatic machine-fact collection, not stock levels:
the agent scans the client and posts a report, GLPI creates or updates the
asset from it.

Generated locally on this box to see the real contents (`--local`, no server
needed). Even a non-root run produced 23 section types:

```
ACCESSLOG BATTERIES BIOS CONTROLLERS(21) CPUS DRIVES(4) ENVS(93)
FIREWALL HARDWARE INPUTS(3) LOCAL_GROUPS(4) LOCAL_USERS(54) MONITORS
NETWORKS(6) OPERATINGSYSTEM PROCESSES(247) SOUNDS STORAGES
USBDEVICES(4) USERS VIDEOS
```

Useful fields are what you would expect — `BIOS` carries `ASSETTAG`,
`SMANUFACTURER`, `SMODEL`; `HARDWARE` carries `MEMORY`, `CHASSIS_TYPE`,
`LASTLOGGEDUSER`; `OPERATINGSYSTEM` carries `FQDN`, `KERNEL_VERSION`,
`INSTALL_DATE`.

Two sections are **not** what you would expect, and `services.glpi-agent`
excludes them by default:

- **`ENVS` — 93 environment variables, names AND values.** Env is a routine
  place for API tokens and credentials; whatever the agent inherits is posted
  to the server and stored in its database.
- **`PROCESSES` — 247 running processes, including command lines.** A process
  snapshot of every client, kept server-side.

Neither says anything about hardware or installed software. So:

```nix
services.glpi-agent.noCategories = [ "environment" "process" ];   # the default
```

Category names are exact, from `glpi-agent --list-categories`. It is **one
flag with a comma-separated value**, not one flag per category — `Config.pm`
lists `no-category` among the multi-value options and splits on `/,+/`
(line 340). Set to `[ ]` to collect everything, which is upstream's default.

**Verified:** re-ran with the flag and parsed the output — `ENVS` 0,
`PROCESSES` 0, the other 22 section types still present.

Note `SOFTWARE` comes back empty on NixOS in either case: the agent looks for
a dpkg/rpm database and there is none.

Clients get `/var/lib/glpi-tls/cert.pem` as their trust anchor. World-readable,
no private key, safe to copy anywhere. See `gl-client-serv.md` §3.5 for Windows
(`msiexec ... SERVER= CA_CERT_FILE= TAG=`, verified against upstream docs but
**not run here**) and §3.6 for macOS (**config path unconfirmed** — find it
after installing; do not take a path on faith).

---

## Part 3 — the setup as it currently runs

Parts 1 and 2 say why it is built this way. This part says what is actually
deployed, read off the running system on 2026-11-01 rather than from the
configuration that was intended. **This is the part that goes stale**: a
decision is only wrong when it is revisited, but a snapshot is wrong the
moment the system moves.

It has moved once since: GLPI went 11.0.12 → 12.0.1 on 2026-11-04. The
version below is the current one; the timings are still the 2026-11-01
readings and are not re-measured.

### The shape of it

```
glpi-agent (root, daily)                        this machine, as a CLIENT
        │  https://nixos.local/   TLS verified against cert.pem
        ▼
nginx  0.0.0.0:443  ─┐
                     ├─ one vhost, two listeners            the SERVER
nginx  127.0.0.1:8080┘
        │
        ├─► GLPI 12.0.1 + PHP-FPM + MariaDB
        │
        └─◄ glpi-inv backend, 127.0.0.1:3200   reads the API over loopback
```

The machine is server and client at once. That is deliberate: the agent's run
leaves the host, resolves mDNS, crosses the firewall, verifies TLS and hits the
inventory endpoint exactly as a remote client would, so the loop is a real
end-to-end test rather than a local shortcut.

### Server

| | |
|---|---|
| listeners | `0.0.0.0:443` (LAN, HTTPS) and `127.0.0.1:8080` (loopback, HTTP) — both observed up |
| why both | `addSSL`, not `forceSSL`: one vhost keeps the routing rules from drifting between the local and LAN copies |
| firewall | `networking.firewall.allowedTCPPorts = [ cfg.lan.port ]`, gated on `lan.enable` — glpi.nix:350 |
| hostname | `services.glpi.hostname`, defaulting to `${config.networking.hostName}.local` → `nixos.local` |
| enabled by | `services.glpi.lan.enable = true` — configuration.nix:392 |

### TLS

| | |
|---|---|
| material | `/var/lib/glpi-tls/cert.pem` (644), `key.pem` (640) |
| generated by | `glpi-cert.service`, oneshot, ordered before nginx; first boot only |
| never | in a derivation — `/nix/store` is world-readable, which is fine for a certificate and fatal for a key |
| subject | `CN=${cfg.hostname}` with `subjectAltName=DNS:nixos.local,DNS:nixos` — glpi.nix:317-318 |
| validity | self-signed, 10 years; `notAfter=Oct 28 06:46:41 2036 GMT` — recorded at generation time (Verification done, below), not re-checked since — there is no `openssl` on this system |
| regeneration | never automatic: a new certificate would invalidate every copy already distributed to clients |
| trust model | clients are handed `cert.pem`; verification stays on |

**Verified live:** `https://nixos.local/` returns **200** with
`ssl_verify_result=0` when curled with `--cacert /var/lib/glpi-tls/cert.pem`.
The IP form is expected to fail, because `192.168.2.3` is deliberately not in
the SAN — that failure is the proof hostname verification is real and not
merely encryption.

### Name resolution

`nixos.local` → `192.168.2.3`, published by avahi (`publish.addresses`, a
separate switch from resolving). `openFirewall` covers UDP 5353.

**`192.168.2.3` is a DHCP lease, not static.** The certificate is bound to the
name rather than the address, so a new lease costs nothing as long as clients
resolve by mDNS. A client using a hosts entry instead would break.

### Client — the agent

| | |
|---|---|
| package | glpi-agent 1.20 |
| runs as | **root** — DMI/SMBIOS tables, block-device serials and the package database are unreadable otherwise; unprivileged it succeeds and reports a near-empty machine |
| tools on PATH | dmidecode, pciutils, usbutils, iproute2, util-linux — a missing tool yields missing fields, not an error |
| invocation | `glpi-agent --conf-file=/nix/store/…-glpi-agent.cfg` |
| schedule | `OnCalendar=daily`, `Persistent=true`, `RandomizedDelaySec=5m` |

The config file is generated and passed explicitly because the agent refuses to
start without one — see `gl-net.md` trap 8. Its entire contents:

```
# Generated by services.glpi-agent - do not edit.
server = https://nixos.local/
ca-cert-file = /var/lib/glpi-tls/cert.pem
no-category = environment,process
```

`no-category` is a privacy decision, not a performance one: the `environment`
category collects environment variables **with values**, a routine place for
credentials, and `process` collects a full process list with command lines.
Neither says anything about installed hardware or software.

Nothing in that file is secret, which is why the store is the right home for
it. `ca-cert-file` and `ca-cert-dir` are mutually exclusive — setting both is
fatal.

### Observed working, end to end

| | |
|---|---|
| agent timer last run | Sun 2026-11-01 **00:03:48** |
| GLPI recorded `last_contact` | 2026-11-01 **00:03:52+01:00** |
| next run | Mon 2026-11-02 00:00:44 |

Four seconds between the timer firing and the check-in landing in GLPI's
database. That single pair is the whole chain confirmed — mDNS, firewall, TLS
verification, the inventory endpoint, the database write — on real data rather
than on a test invocation.

### Consumer — the dashboard

`glpi-inv` reads the result back out through GLPI's High-Level API:

| | |
|---|---|
| listens | `127.0.0.1:3200`, systemd user unit, enabled |
| reads | `http://localhost:8080` — the loopback listener, not the LAN one |
| as | GLPI user `inv`, profile `inv` (central interface), Root entity, recursive |
| rights | `agent` and `entity` READ, plus whatever else the Read-Only copy carried |

It uses loopback deliberately: it runs on the same host as GLPI, so the LAN
listener and its certificate are irrelevant to it. See `gl-rights.md` in this directory for what
the account needs and why that took three attempts.

**Cross-check worth noting:** glpi-inv calls an agent stale after 26 hours,
chosen to tolerate the timer's drift. The drift here is bounded by
`RandomizedDelaySec=5m`, so 26 hours is generous rather than tight — the
margin is real but larger than this configuration strictly needs.

### Not done

- **No second client.** The only agent is this machine. Every claim about the
  client-server path is therefore true for a loopback-resolvable host that
  happens to go out over the network; nothing has been proven from a genuinely
  separate machine.
- **Windows and macOS instructions are untested** — written from upstream
  documentation with no such host available. `gl-client-serv.md` §3.5-3.6 mark
  which parts those are, including one path it could not confirm.
- **The DHCP lease is not pinned** in the router, or at least that was never
  confirmed here.

## Traps

**1. `php.ini` comments must use `;`, not `#`.**
PHP 7 dropped `#`. A `#` line is parsed as a *setting*, and a quote inside it
is a hard syntax error:

```
PHP:  syntax error, unexpected '"' in .../php.ini on line 50
```

The build succeeded; only running `php --version` showed it. Nix's
`extraConfig` will happily emit whatever you write.

**2. Never add `location ~ \.php$ { return 404; }`.**
It looks like obvious hardening and it breaks GLPI outright. GLPI 12 still
addresses legacy entry points by `.php` URL — the login form posts to
`/front/login.php` — but those are **routes handled by `public/index.php`, not
files on disk**. nginx tries regex locations **before** the `location /`
prefix and takes the first match, so the guard answers first and `try_files`
never runs.

Symptom: the login page renders perfectly, then 404s the moment you submit.
`/front/inventory.php` would break every agent the same way.

The reasoning that produced it was: `index.php` is the only `.php` *file* in
`public/`, so any other `.php` request must be illegitimate. True of files,
false of URLs — and the right conclusion from that same evidence is the
opposite one.

It also survived testing because the end-to-end check used PHP's **built-in
server**, which has no location blocks at all. Fetching `/` passed because
`try_files` rewrites it to `/index.php` internally. One
`curl -I http://localhost:8080/front/login.php` against the real nginx would
have caught it.

**3. `listen ... ssl` does not emit a certificate.**
nixpkgs' nginx module derives
`hasSSL = onlySSL || addSSL || forceSSL` (`nginx/default.nix:336`) and emits
`ssl_certificate` only under that flag (line 483). A manual `listen` entry with
`ssl = true` sets none of them, so the generated config had a TLS listener and
**no certificate** — nginx refuses to start.

Fix is `addSSL = true`. **Not `forceSSL`**: it filters the listen list down to
TLS entries (`hostListen`, line 380), silently deleting the loopback HTTP
listener and redirecting it to a hostname `127.0.0.1` is not.

Caught by grepping the generated `nginx.conf` for `ssl_certificate` and getting
nothing back.

**4. nginx's config test runs as the `nginx` user, so `0600 root` keys fail.**
The intuitive reasoning — "nginx's master runs as root, root-only is enough" —
is wrong on NixOS. The generated unit sets `User=nginx` for the **whole**
service, including the `ExecStartPre` that runs `nginx -t`:

```
nginx: [emerg] cannot load certificate key "/var/lib/glpi-tls/key.pem":
BIO_new_file() failed (SSL: ... Permission denied ...)
nginx: configuration file ... test failed
```

Then `start-limit-hit` after a few restarts. `0640 root:nginx` is the fix, with
the directory `0755` so nginx can traverse it. Enforce it **outside** the
"generate if missing" branch, or an existing key keeps its old mode forever.

**5. `database:install` needs `--db-password=` with an empty value.**
The account uses MariaDB's `unix_socket` plugin and has no password, but GLPI
validates the *option* and rejects null:

```
--db-password option value cannot be null.
```

`--no-interaction` means it cannot prompt either. An empty string passes; the
server ignores it because auth is by OS user, which `sudo -u glpi` satisfies.

Keep `--no-interaction` for an unrelated reason: both GLPI confirmations
default to **Yes**, including *"Do you want to reconfigure database?"*, which
overwrites a working install. Non-interactively that branch is skipped and
`execute()` returns `ERROR_DB_CONFIG_ALREADY_SET`, so clobbering requires
typing `--reconfigure` or `--force` on purpose.

`--no-telemetry` is **not** needed: `install/empty_data.php:766` registers the
crontask with `state => CronTask::STATE_DISABLE`. Change it later under
**Setup → Automatic actions → telemetry**; it is just that task's `state`
column (`Telemetry::enable()` is a one-row `UPDATE`).

**6. `/etc/nixos` is a git repo, and git flakes ignore untracked files.**
A new `glpi.nix` or `glpi-agent.nix` is **invisible** until `git add`-ed, and
the rebuild fails claiming the file does not exist while it sits right there.

**7. The default accounts are checked by hash, and deactivating counts.**
`User::checkDefaultPasswords()` queries with `is_active => 1, is_deleted => 0`
and verifies each stored hash against the known default, so **Active → No**
clears the banner exactly as a password change does. The defaults are `glpi`/
`glpi`, `tech`/`tech`, `normal`/`normal` and `post-only`/**`postonly`** (no
hyphen).

The banner only renders for users with `config` UPDATE right
(`Central.php:580`) — its absence on a lesser account is not an all-clear.

---

## Verification done

A record of what was executed **while building this**, dated to that work —
not a statement about the system today. Values here are the ones observed
then, and are deliberately not updated: the point of the section is
evidence that each step was actually run rather than assumed. For current
state, Part 3.

| check | result |
|---|---|
| `php --version` | 8.4.27, all 19 required extensions, ini settings applied |
| `system:check_requirements` | 23 OK, 0 errors, 0 warnings |
| `database:install` | "Installation done." against a throwaway MariaDB |
| `GET /` | 200, 86 KB, `<title>Authentication - GLPI</title>` |
| `POST /front/login.php` | 302 → `/front/central.php`, session cookie set |
| `GET /front/central.php` | 200, 212 KB, `<title>Standard interface - GLPI</title>` |
| `system:status` | `glpi: OK`, `db: OK` |
| store-relocation | 14 state dirs created outside the store |
| cert generation | SAN + 2036 expiry confirmed by running it |
| `nixos.local` | resolves to 192.168.2.3 |

The throwaway MariaDB and test web server were stopped and their data deleted;
the admin session opened to test login was logged out.

### LAN side, after trap 4 was fixed

| check | result |
|---|---|
| `nginx`, `glpi-cert`, `phpfpm-glpi`, `mysql`, `avahi-daemon` | all active |
| listeners | `0.0.0.0:443` and `127.0.0.1:8080` both up |
| key permissions | `640 root:nginx` — the unconditional `chmod` corrected the pre-existing 0600 key |
| `https://nixos.local/` with `--cacert` | **200**, `ssl_verify_result=0` |
| `https://192.168.2.3/` with the same CA | **refused** — the IP is not in the SAN, so hostname verification is genuinely enforced, not merely encryption |
| `http://localhost:8080/` | 200 — the loopback listener survived, i.e. `addSSL` not `forceSSL` was the right call |
| `GET /Inventory` | **405**, not 403 — the enabled-check passed; `InventoryController.php:78` returns 405 for a non-POST, which is the correct response for a live endpoint |

### The agent loop — verified end to end

```
[info] target server0: server https://nixos.local/
[info] sending prolog request to server0
[info] server0 answer shows it supports GLPI Agent protocol
[info] sending contact request to server0
[info] running task Inventory
[info] New inventory from nixos-2026-10-31-08-36-00 for server0
Deactivated successfully.      exit 0, 28K outgoing IP traffic
```

The protocol-handshake line is the one that matters: the agent resolved the
mDNS name, passed the firewall, **verified the self-signed certificate against
`cert.pem`**, and was accepted by the inventory endpoint. Timer armed daily
with `Persistent = true`.

The `Use of uninitialized value ... PartNumber/Samsung.pm` lines are harmless
upstream Perl noise, not a symptom of anything here.

**trap 8 — the agent demands a config file that no NixOS system has.**
Passing every setting on the command line is not enough; the file check runs
first and kills it:

```
Execution failure:.
Config: non-existing file /etc/agent.cfg
```

With no `--conf-file` it falls back to `confdir/agent.cfg`
(`Config.pm:231`), and nixpkgs builds with `SYSCONFDIR=/etc`. The module
therefore generates a real `key = value` file with `pkgs.writeText` and passes
`--conf-file`. Same idiom documented for non-NixOS clients, so the two cannot
drift; nothing in it is secret, so the store is the right home for it.

This one was hit by hand earlier in the session, worked around with an empty
`--conf-file`, and then not carried into the module — which is how it reached
a switch.

## Files

| path | what |
|---|---|
| `/etc/nixos/glpi.nix` | package + module: PHP env, nginx, MariaDB, TLS, cron |
| `/etc/nixos/glpi-agent.nix` | the agent module, importable on other NixOS hosts |
| `/etc/nixos/flake.nix` | `nixpkgs-php` input + `specialArgs` |
| `/var/lib/glpi` | state, `0750 glpi:glpi` |
| `/var/lib/glpi-tls` | `cert.pem` 0644, `key.pem` 0640 root:nginx |
