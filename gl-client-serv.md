# GLPI server + client infrastructure — setup manual

The ordered procedure for building this from nothing: a GLPI server on NixOS,
reachable over TLS on the LAN, with inventory agents on clients reporting into
it. Each step says how to confirm it worked before moving to the next.

This file is the **how**. The **why** — every design decision, and the eleven
traps that produced them — is `gl-net.md` in this directory; where a step has
a non-obvious reason it is cross-referenced rather than re-argued here.

Everything marked **verified** was executed on this machine and its output
read. Everything marked **untested** was not, and says so.

- **Server:** GLPI 12.0.1 on NixOS 26.05 — nginx → php-fpm (PHP 8.4.27) →
  MariaDB 11.4.12 over a unix socket
- **Clients:** glpi-agent 1.20, verifying a self-signed certificate
- **Reach:** `0.0.0.0:443` HTTPS for the LAN, `127.0.0.1:8080` HTTP for the host
- **Tested at:** one server, one client (the same machine). Multi-host is
  written from the modules and upstream docs — see *Not proven* at the end.

---

## 0. What you are building

```
         CLIENTS                                    SERVER
                                             ┌──────────────────────────┐
 glpi-agent (root, daily)                    │ nginx  0.0.0.0:443  TLS  │
   nixos.local  ──TLS──────────────────────► │ nginx  127.0.0.1:8080    │
   trust anchor: cert.pem                    │   one vhost, two         │
                                             │   listeners              │
 glpi-agent on other hosts                   │        │                 │
   (same cert, same config keys)  ─────────► │   php-fpm → GLPI 12.0.1  │
                                             │        │                 │
                                             │   MariaDB (unix socket)  │
                                             └──────────────────────────┘
                                                      ▲
                                             glpi-cron.timer, minutely
```

Parts list, and where each one is defined:

| part | where |
|---|---|
| GLPI package + nginx + php-fpm + MariaDB + TLS + cron | `/etc/nixos/glpi.nix` — hand-written; nixpkgs has no `glpi` package and no `services.glpi` |
| the inventory agent unit | `/etc/nixos/glpi-agent.nix` — hand-written; nixpkgs has `pkgs.glpi-agent` but no `services.glpi-agent` |
| PHP 8.4.27 (a second nixpkgs input) | `/etc/nixos/flake.nix` — see `gl-net.md` Part 1 for why |
| name resolution | `services.avahi` in `configuration.nix` |
| turning it all on | `configuration.nix:387-402` |

Both `.nix` files are importable unchanged on another NixOS host.

---

## 1. Decide three things first

Changing any of these after clients exist costs work, so settle them now.

**The server's name.** `services.glpi.hostname`, default
`${config.networking.hostName}.local` → `nixos.local` here. The TLS
certificate is issued for this name and nothing else, so every client must be
able to resolve it. A `.local` name resolves over mDNS with no DNS server to
run, which is why it was chosen.

**Name, not address.** `192.168.2.3` here is a DHCP lease and is deliberately
**not** in the certificate's SAN. Binding to the name means a new lease costs
nothing. If you bind to an address instead you must reissue the certificate
every time the lease moves, and reissuing invalidates every copy already
distributed. Pin the lease in the router as well — belt and braces.

**LAN or loopback only.** `services.glpi.lan.enable = false` gives you GLPI on
`127.0.0.1:8080` and nothing else: no open port, no certificate, no remote
clients. Start there if you only want the web interface; the LAN switch is
additive and can be flipped later without touching anything else.

---

## 2. Server

### 2.1 Place the two modules — and `git add` them

Copy `glpi.nix` and `glpi-agent.nix` into `/etc/nixos/`, then:

```sh
cd /etc/nixos
git add glpi.nix glpi-agent.nix
```

**The `git add` is not optional.** `/etc/nixos` is a git repository, and git
flakes ignore untracked files. A new module is invisible to the rebuild, which
then fails claiming the file does not exist while it sits right there in the
directory. (`gl-net.md` trap 6.)

### 2.2 Import and enable

In `configuration.nix`:

```nix
imports = [
  ./glpi.nix
  ./glpi-agent.nix
];

services.glpi.enable = true;
services.glpi.lan.enable = true;      # HTTPS on the LAN + the firewall port
```

If you took the PHP-pin route, `flake.nix` needs the second input and
`specialArgs = { inherit inputs; }` so `glpi.nix` can reach it. Check whether
the main pin has caught up first: when `pkgs.php84` is ≥ 8.4.27, delete the
input and the `phpPkgs` line in `glpi.nix` and take PHP from `pkgs`.

### 2.3 Publish the hostname over mDNS

Resolving *other* hosts' `.local` names and *announcing your own* are separate
switches, and only the first is on by default. Without this,
`getent hosts nixos.local` returns nothing and no client can find the server.

```nix
services.avahi = {
  enable = true;
  nssmdns4 = true;
  openFirewall = true;              # UDP 5353
  publish = { enable = true; addresses = true; workstation = true; };
};
```

### 2.4 Rebuild

```sh
sudo nixos-rebuild switch --flake /etc/nixos#nixos --max-jobs 2 --cores 2
```

The job limits are a memory guard, not a style choice: this box has 7.6 GB and
a parallel from-source build has OOM-killed the nix-daemon here before.

On this first switch `glpi-cert.service` generates the certificate and key
under `/var/lib/glpi-tls` — ordered **before nginx**, because nginx refuses to
start when `ssl_certificate` points at a file that does not exist.

Confirm the services came up:

```sh
systemctl is-active nginx phpfpm-glpi mysql avahi-daemon glpi-cert
ss -ltn | grep -E ':443|:8080'
```

Expect five `active` and two listeners — `0.0.0.0:443` and `127.0.0.1:8080`.
**Verified here.**

### 2.5 Create the schema — once, by hand

A rebuild deliberately does **not** do this. Creating the schema is a one-time
action with a destructive `--force` variant; no rebuild should ever be able to
take it.

```sh
sudo -u glpi glpi-console database:install \
  --db-name=glpi --db-user=glpi --db-password= --no-interaction
```

Three things about that command line, all of which bite:

- **`--db-password=` with an empty value is required and is not a typo.** The
  account uses MariaDB's `unix_socket` plugin and has no password, but GLPI
  validates the *option* and rejects a null: *"--db-password option value
  cannot be null."* An empty string passes validation and MariaDB ignores it,
  because authentication is by OS user — which `sudo -u glpi` satisfies.
- **Keep `--no-interaction`,** for a reason unrelated to prompts. Both of
  GLPI's confirmations default to **Yes**, including *"Do you want to
  reconfigure database?"*, which overwrites a working install. Non-
  interactively that branch is skipped and the command returns
  `ERROR_DB_CONFIG_ALREADY_SET`, so clobbering an existing install requires
  typing `--reconfigure` or `--force` on purpose.
- **`--no-telemetry` is not needed.** `install/empty_data.php` registers the
  telemetry crontask already disabled.

Expect `Installation done.` **Verified here.**

Then, optionally, enable per-user timezones:

```sh
sudo -u glpi glpi-console database:enable_timezones
```

On a fresh MariaDB this refuses with *"Timezones seems not loaded, see
<https://glpi-install.readthedocs.io/en/latest/timezones.html>"*. The server's
timezone tables are empty by default, and `DbTimezones.php` resolves named
zones through `CONVERT_TZ`, which reads them. Load them and grant the GLPI
account read access, then re-run:

```sh
sudo sh -c 'mariadb-tzinfo-to-sql /etc/zoneinfo | mariadb mysql'
sudo mariadb -e "GRANT SELECT ON mysql.time_zone_name TO 'glpi'@'localhost';"
```

Those are the two steps the page in that error message documents.
**Verified here** on 2026-11-06: `mariadb-tzinfo-to-sql /etc/zoneinfo`
(resolving to `tzdata-2026c`) loaded **1791** rows into
`mysql.time_zone_name`, and `database:enable_timezones` then reported
*Timezone usage has been enabled.*

The grant is needed even though the account is otherwise unprivileged outside
its own schema: `mysql.*` is not covered by the `glpi.*` grant that
`ensureUsers` creates.

Note this is separate from PHP's own `date.timezone`, which `glpi.nix` sets
from `config.time.timeZone` (`Europe/Amsterdam`) and which is what GLPI's
status page checks. Skipping the step above costs only per-user timezone
selection, not a clean status page.

### 2.6 First login, and kill the default passwords

Browse to `http://localhost:8080/` and log in as `glpi` / `glpi`.

GLPI ships **four** accounts with published defaults:

| login | password |
|---|---|
| `glpi` | `glpi` |
| `tech` | `tech` |
| `normal` | `normal` |
| `post-only` | `postonly` — **no hyphen** |

Change the password on the one you will use and disable the rest. Either in
the UI, or:

```sh
sudo -u glpi glpi-console user:disable tech
sudo -u glpi glpi-console user:disable normal
sudo -u glpi glpi-console user:disable post-only
```

Two things worth knowing about GLPI's own warning banner:

- It is checked **by hash against the known defaults**, with
  `is_active => 1, is_deleted => 0` — so setting **Active → No** clears it
  exactly as a password change does. Disabling is a real fix, not a cosmetic
  one.
- It renders only for users holding `config` UPDATE right. **Its absence on a
  lesser account is not an all-clear.**

### 2.7 Enable native inventory

**It ships disabled, and this is the step most easily missed.** Until it is
on, `/Inventory` and `/front/inventory.php` both return **403** and every
agent fails — `InventoryController.php` throws `AccessDeniedHttpException`
unless `enabled_inventory == 1`.

```sh
sudo -u glpi glpi-console config:set --context=inventory enabled_inventory 1
```

The UI equivalent exists, but the console form is reproducible and is what
belongs in a runbook.

### 2.8 Confirm the server before touching any client

```sh
# the application itself
sudo -u glpi glpi-console system:status            # expect glpi: OK, db: OK
sudo -u glpi glpi-console system:check_requirements # expect 0 errors, 0 warnings

# the loopback listener
curl -s -o /dev/null -w '%{http_code}\n' http://localhost:8080/        # 200

# the LAN listener, with hostname verification ON
curl -s -o /dev/null -w '%{http_code} verify=%{ssl_verify_result}\n' \
  --cacert /var/lib/glpi-tls/cert.pem https://nixos.local/             # 200 verify=0

# the inventory endpoint
curl -s -o /dev/null -w '%{http_code}\n' http://localhost:8080/Inventory
```

That last one should return **405, not 403**. 405 means the enabled-check
passed and the endpoint is live — it is simply refusing a GET, which is the
correct answer for an endpoint that only accepts POST. A 403 means step 2.7
did not take. **All four verified here.**

One more check worth running deliberately:

```sh
curl -s -o /dev/null -w '%{http_code}\n' --cacert /var/lib/glpi-tls/cert.pem \
  https://192.168.2.3/
```

This should **fail**. The address is not in the certificate's SAN, and that
failure is the proof hostname verification is genuinely enforced rather than
encryption alone.

---

## 3. Clients

### 3.1 Get the trust anchor off the server

```sh
cp /var/lib/glpi-tls/cert.pem  ./glpi-server-cert.pem
```

`cert.pem` is mode 0644 and contains **no private key**. It is safe to email,
put on a share, or commit to a repository. `key.pem` is 0640 root:nginx and
never leaves the box.

**Use the certificate; do not use `--no-ssl-check`.** Agent 1.20 supports
`--ca-cert-file` and `--ssl-fingerprint`, so a self-signed server gets both
encryption *and* authentication. `--no-ssl-check` accepts any certificate at
all: protection against a passive listener, none whatever against an active
one. On WPA2-PSK — where every device holding the passphrase can decrypt its
peers' traffic — that difference is the entire reason TLS is here. What is at
risk is not really the inventory data; it is the 128-character `glpi_<hash>`
session cookie, whose replay is a full admin session.

### 3.2 Config format, once, for every platform

All platforms share one parser (`GLPI/Agent/Config.pm`), so the file format and
the keys are identical everywhere: `key = value`, keys matching the long
option names.

```
server = https://nixos.local/
ca-cert-file = /etc/glpi-agent-ca.pem
tag = laptop
no-category = environment,process
```

- `server` is the **site root**. The agent appends its own path — do not put
  `/Inventory` or `/front/inventory.php` here.
- `ca-cert-file` and `ca-cert-dir` are **mutually exclusive**; setting both is
  a fatal error.
- `no-category` is **one key with a comma-separated value**, not one line per
  category. Valid names come from `glpi-agent --list-categories`.

On the two excluded categories: `environment` collects 93 environment
variables with **values** — a routine place for API tokens — and `process`
collects 247 running processes **with command lines**, as a server-side
snapshot of every client. Neither says anything about hardware or installed
software. They are upstream's default and are excluded here on purpose. Set
`noCategories = [ ]` to collect everything.

### 3.3 A NixOS client

Copy `glpi-agent.nix` to the client and import it:

```nix
imports = [ ./glpi-agent.nix ];

environment.etc."glpi-agent-ca.pem".source = ./glpi-server-cert.pem;

services.glpi-agent = {
  enable = true;
  server = "https://nixos.local/";
  caCertFile = "/etc/glpi-agent-ca.pem";
  tag = "laptop";                        # optional, groups assets
};
```

Placing the certificate with `environment.etc` rather than copying it by hand
is what makes it survive a rebuild.

The unit runs the agent as **root**, with `dmidecode`, `pciutils`, `usbutils`,
`iproute2` and `util-linux` on its PATH. Both are deliberate and both failure
modes are silent: unprivileged, the agent starts, **succeeds, and reports a
near-empty machine**; with a helper tool missing, the asset simply arrives with
fields absent. Neither produces an error.

### 3.4 A non-NixOS Linux client

Install the agent from the distribution or from upstream's releases, then write
`/etc/glpi-agent/agent.cfg` with the block from 3.2. Run `glpi-agent` once by
hand to check, then let the packaged timer or cron job take over.

On NixOS specifically the agent **demands a config file that no NixOS system
has**: with no `--conf-file` it falls back to `confdir/agent.cfg`, and nixpkgs
builds it with `SYSCONFDIR=/etc`, so it dies with *"Config: non-existing file
/etc/agent.cfg"*. Passing every setting on the command line is not enough —
the file check runs first. That is why the module generates a real file and
passes `--conf-file`; packaged distributions ship one already.

### 3.5 Windows — **untested**, written from upstream docs

MSI from https://github.com/glpi-project/glpi-agent/releases . Copy the
certificate to its destination **before** installing. Property names are
case-sensitive and take no spaces around `=`. From **cmd, not PowerShell**:

```
msiexec /i GLPI-Agent-1.21-x64.msi /quiet ^
  SERVER=https://nixos.local/ ^
  CA_CERT_FILE=C:\ProgramData\GLPI-Agent\glpi-server-cert.pem ^
  TAG=desktop
```

`NO_SSL_CHECK=1` exists; see 3.1 for why not to use it.

Upstream ships 1.21 while nixpkgs has 1.20. Mixed versions are fine — the
inventory protocol is stable across minor releases.

Windows 10 1703+ resolves `.local` over mDNS natively. If it does not, add a
hosts entry, or use the server's IP and accept that the certificate must then
be reissued with that IP in its SAN.

### 3.6 macOS — **untested, and one detail unconfirmed**

`.pkg` from the same releases page. The config *keys* are the ones in 3.2.

**The config file path could not be confirmed.** After installing, find it:

```sh
sudo find / -name "agent.cfg" -path "*glpi*" 2>/dev/null
```

and write the block from 3.2 into it. Do not take a path on faith here; the
rest of this section was verified against upstream documentation and this one
line was not.

macOS resolves `.local` natively via Bonjour, so the hostname needs no work.

### 3.7 The server as its own client

Worth doing even on a single-machine install, and not as a shortcut: the
agent's run leaves the host, resolves mDNS, crosses the firewall, verifies TLS
and hits the inventory endpoint exactly as a remote client would. It is a real
end-to-end test of the whole path.

```nix
services.glpi-agent = {
  enable = true;
  server = "https://${config.services.glpi.hostname}/";
  caCertFile = "${config.services.glpi.lan.tlsDir}/cert.pem";
};
```

Written against the options rather than literal values, so a hostname or path
change does not have to be made twice.

---

## 4. Verify the loop end to end

On the client, force a run — `--force` sends even when the agent thinks
nothing has changed:

```sh
glpi-agent --server=https://nixos.local/ \
  --ca-cert-file=/etc/glpi-agent-ca.pem --force
```

The line that matters in the output is the handshake:

```
[info] target server0: server https://nixos.local/
[info] sending prolog request to server0
[info] server0 answer shows it supports GLPI Agent protocol
[info] running task Inventory
[info] New inventory from nixos-2026-10-31-08-36-00 for server0
```

`server0 answer shows it supports GLPI Agent protocol` means the agent
resolved the name, passed the firewall, **verified the self-signed certificate
against `cert.pem`**, and was accepted by the inventory endpoint. Everything
after that is GLPI's problem, not the network's.

Then on the server, check **Assets → Computers** for the machine, and compare
the timer's last run against what GLPI recorded:

```sh
systemctl list-timers glpi-agent.timer --no-pager
```

**Verified here:** timer fired `00:03:48`, GLPI's `last_contact` read
`00:03:52+01:00`. Four seconds between the two is the entire chain confirmed —
mDNS, firewall, TLS verification, the inventory endpoint, the database write —
on real data rather than a hand-run test.

`Use of uninitialized value ... PartNumber/Samsung.pm` in the agent's output is
harmless upstream Perl noise.

On NixOS clients, note that `SOFTWARE` comes back empty in either
configuration: the agent looks for a dpkg or rpm database and there is none.

---

## 5. Running it

### Routine commands

```sh
sudo -u glpi glpi-console <command>          # the wrapper sets the three env vars
sudo -u glpi glpi-console system:status
sudo -u glpi glpi-console database:check_schema_integrity
sudo -u glpi glpi-console security:session:list
sudo -u glpi glpi-console cache:clear
```

Use the wrapper, never `php bin/console` directly. `GLPI_CONFIG_DIR`,
`GLPI_VAR_DIR` and `GLPI_MARKETPLACE_DIR` must agree across php-fpm, the cron
unit and the console — a console run with different paths writes a
`config_db.php` the web process cannot see.

### The scheduler

`glpi-cron.timer` runs `front/cron.php` minutely, `Persistent = true`.
GLPI 12 ships **no** cron console command, which is why it is the legacy
entry point. `Persistent` matters on a laptop: without it, notifications and
scheduled tasks never fire for the hours the machine is asleep.

### Backup

There is **no `database:dump` console command** — checked against the 12.0.1
command list. Two things need copying:

```sh
sudo mariadb-dump --single-transaction glpi > glpi-$(date +%F).sql
sudo tar czf glpi-state-$(date +%F).tar.gz /var/lib/glpi
```

`/var/lib/glpi/config/config_db.php` holds the database credentials **and the
encryption key** — which is why that directory is 0750 and why a backup of it
is as sensitive as the database itself. `/var/lib/glpi/files` holds every
uploaded document. The GLPI source needs no backup; it is in the store and
rebuilt from the pinned hash.

Also back up `/var/lib/glpi-tls`. Regenerating the certificate invalidates
every copy already distributed to clients, so losing it means revisiting every
agent.

### Upgrading GLPI

```sh
sudo -u glpi glpi-console maintenance:enable
# bump `version` and `hash` in glpi.nix, then:
sudo nixos-rebuild switch --flake /etc/nixos#nixos --max-jobs 2 --cores 2
sudo -u glpi glpi-console database:update        # run this interactively and read it
sudo -u glpi glpi-console maintenance:disable
```

Take the database backup **before** `maintenance:enable`, not after.
`database:update` migrates the schema; the rebuild alone only changes the code
and leaves GLPI reporting a schema mismatch. If anything downstream reads
GLPI's API, read `gl-upgrade.md` first — the version coupling is real and some
of its failures are silent.

---

## 6. Reference

### Ports

| port | bound to | opened by | what |
|---|---|---|---|
| 443/tcp | `0.0.0.0` | `networking.firewall.allowedTCPPorts`, gated on `lan.enable` | HTTPS, browsers and agents |
| 8080/tcp | `127.0.0.1` | nothing — loopback is unreachable from the network whatever the firewall says | HTTP, this host only |
| 5353/udp | all | `services.avahi.openFirewall` | mDNS, `.local` resolution |

### Files

| path | mode | what |
|---|---|---|
| `/etc/nixos/glpi.nix` | — | package + module: PHP env, nginx, MariaDB, TLS, cron |
| `/etc/nixos/glpi-agent.nix` | — | the agent module, importable on any NixOS host |
| `/var/lib/glpi` | 0750 glpi:glpi | all mutable state |
| `/var/lib/glpi/config` | 0750 glpi:glpi | `config_db.php` — credentials **and** encryption key |
| `/var/lib/glpi/files` | 0750 glpi:glpi | uploads, cache, logs, sessions |
| `/var/lib/glpi-tls/cert.pem` | 0644 | the trust anchor handed to clients |
| `/var/lib/glpi-tls/key.pem` | 0640 root:nginx | never leaves the machine |

`key.pem` is **not** 0600 root, and the intuitive reasoning for 0600 — "nginx's
master runs as root" — is wrong on NixOS. The generated unit sets `User=nginx`
for the whole service including the `ExecStartPre` that runs `nginx -t`, which
then cannot read the key and aborts the start. (`gl-net.md` trap 4.)

### Units

| unit | type | when |
|---|---|---|
| `glpi-cert.service` | oneshot, `RemainAfterExit` | before nginx; generates only if missing |
| `phpfpm-glpi.service` | the pool | — |
| `glpi-cron.timer` | minutely, `Persistent` | GLPI's scheduler |
| `glpi-agent.timer` | daily, `Persistent`, `RandomizedDelaySec=5m` | inventory submission |

---

## 7. Troubleshooting, by symptom

| symptom | cause |
|---|---|
| rebuild says `glpi.nix` does not exist, and it plainly does | untracked in `/etc/nixos`'s git repo; flakes ignore untracked files — `git add` it |
| nginx fails: `cannot load certificate key ... Permission denied` | `key.pem` is 0600 root. `nginx -t` runs as the nginx user — 0640 root:nginx, directory 0755 |
| nginx fails and the generated config has a TLS listener with no certificate | `listen ... ssl = true` alone sets nothing. Needs `addSSL = true` — **not** `forceSSL`, which deletes the loopback listener |
| login page renders perfectly, then 404s the moment you submit | a `location ~ \.php$ { return 404; }` block. GLPI addresses legacy entry points by `.php` URL, but they are **routes through `public/index.php`**, not files. nginx matches regex locations before the `location /` prefix. Delete the block |
| `database:install` fails: *option value cannot be null* | `--db-password=` with an empty value is required, even with socket auth |
| every agent gets **403** from the inventory endpoint | native inventory is still disabled — step 2.7 |
| every agent gets **404** from the inventory endpoint | nginx is blocking `.php` routes — see the 404-on-submit row |
| agent dies: `Config: non-existing file /etc/agent.cfg` | no config file. Generate one and pass `--conf-file`; command-line settings are not enough |
| agent dies on TLS | wrong certificate, or a path that does not exist on the client. Check it resolves the **name** in the SAN, not an address |
| agent fatal: ca-cert-file and ca-cert-dir | mutually exclusive; set exactly one |
| asset arrives nearly empty, agent exited 0 | agent not running as root |
| asset arrives with fields missing | a helper tool off the unit's PATH — dmidecode, pciutils, usbutils, iproute2, util-linux |
| agent exits 0 and no asset ever appears | GLPI rejected the submission. `RefusedEquipment` is not exposed by either API; see `gl-sql-plan.md` |
| client cannot resolve `nixos.local` | `services.avahi.publish.addresses` not set on the server — resolving and announcing are separate switches |
| PHP syntax error on a line that is a comment | `php.ini` comments must use `;`. PHP 7 dropped `#`, so a `#` line is parsed as a setting and a quote in it is fatal |
| GLPI warns about default passwords after you disabled the accounts | it should not — the check honours `is_active`. If it persists, one of the four is still active |
| no default-password warning at all | the banner renders only for users with `config` UPDATE right. Not an all-clear |

---

## Not proven

Stated plainly so nothing here reads as more tested than it is.

- **No second client.** The only agent is the server itself. Every claim about
  the client-server path is true for a host that resolves its own mDNS name
  and goes out over the network to reach itself — which exercises the whole
  chain, but is not a genuinely separate machine.
- **Windows (3.5) and macOS (3.6) are untested**, written from upstream
  documentation with no such host available. 3.6 flags one path it could not
  confirm at all.
- **The DHCP lease is not pinned** in the router, or at least that was never
  confirmed here.
- **Certificate expiry** (`notAfter=Oct 28 06:46:41 2036 GMT`) was recorded at
  generation time and has not been re-checked; there is no `openssl` on this
  system outside the generating unit.

## See also

| file | what |
|---|---|
| `gl-net.md` | the reasoning behind every decision above, and the eleven traps in full |
| `gl-upgrade.md` | what a GLPI upgrade breaks downstream, and in what order |
| `gl-rights.md` | profile rights for a read-only API service account |
| `gl-db-api.md` | what defines GLPI's API surface — code, not database |
| `gl-sql-plan.md` | the two things no GLPI API exposes, and what reaching them costs |
