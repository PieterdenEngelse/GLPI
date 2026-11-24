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

### What "over a unix socket" and "state in /var/lib/glpi" actually mean

Two phrases in that summary carry most of the design, so here they are in full.

**GLPI reaches MariaDB over `/run/mysqld/mysqld.sock`**, not over TCP, and the
account has **no password at all**. `services.mysql.ensureUsers` creates `glpi`
with MariaDB's `unix_socket` plugin, which authenticates by the peer credential
of the connecting process: being the `glpi` OS user *is* the credential. That is
why `database:install` takes an empty `--db-password=`, why there is no secret
to rotate or leak, and why routine administration must go through
`sudo -u glpi` — the same command as another user cannot authenticate at all.

Two consequences worth stating plainly:

- **PHP needs to be told the socket path.** `mysqli` resolves the host
  `localhost` to a socket at a compiled-in default location, which is not where
  NixOS puts MariaDB's. Hence `mysqli.default_socket = /run/mysqld/mysqld.sock`
  in `glpi.nix`. Without it the install fails to reach the database at all.
- **MariaDB will bind TCP on every interface unless told not to.** nixpkgs'
  `services.mysql` sets no `bind-address` — only `datadir` and
  `port = mkDefault 3306` under `settings.mysqld` — so MariaDB falls back to
  its own default and listens on `0.0.0.0:3306`. Nothing could reach it, since
  the firewall accepts only `tcp dport 443` and `udp dport 5353` (read out of
  `/nix/store/…-firewall-start/bin/firewall-start`), but that left the firewall
  as the only control in the way.

  `glpi.nix` therefore sets **`services.mysql.settings.mysqld.bind-address =
  "127.0.0.1,::1"`**. Not `skip-networking`: both loopbacks are equally
  unreachable off-host, and keeping TCP there leaves ad-hoc tooling a path
  instead of removing a capability that is not in the way.

  **Name both, and know why.** A single IPv4 `bind-address` does not merely add
  v4 — it *drops* the v6 bind. MariaDB answers on `[::]:3306` by default, and
  setting `127.0.0.1` alone removed it; checking only the v4 line after such a
  change under-reports what happened. That matters because `getent ahosts
  localhost` returns **`::1` first** here, so a client told to reach
  `localhost` over TCP tries v6 before v4 — and against a v4-only bind its
  first attempt is `ECONNREFUSED`, which reads as a dead database rather than
  a wrong address family. Clients that fall back to v4 are fine; not all do.

  The comma-separated form is MariaDB's own, verified against this build rather
  than its documentation — `mariadbd --help --verbose`: *"IP address to bind to.
  Several addresses may be specified, separated by a comma (,)."*

  The coherent choices are **both loopbacks or `skip-networking`**. v4-only is
  the one with no rationale behind it.

  Do not try to confirm this from the server itself — traffic to the host's own
  address goes over loopback, which the firewall does not filter, so the probe
  succeeds either way and proves nothing. Check the bind address with
  `ss -ltn | grep 3306`, not a connection test.

  The listener was never a credential exposure on its own: the `unix_socket`
  plugin needs a peer credential the kernel supplies only over a local socket,
  so such an account **cannot** authenticate over TCP at all. That protection
  ends the moment a password-authenticated account exists — which is the kind
  of change nobody revisits a bind-address for.

**The source is read-only and every writable path is redirected.** The GLPI
tree lives at `/nix/store/…-glpi-12.0.1`, mode `r-xr-xr-x`, on a mount whose
options are `ro,nosuid,nodev` — read-only by the mount, not merely by
permissions. GLPI, however, writes *inside its own source tree* by default, so
that would be fatal if it could not be redirected.

It can. `SystemConfigurator.php:190` defines every path constant as:

```php
if (!defined($name) && ($value = getenv($name)) !== false) { define($name, $value); }
```

so each is overridable by environment variable. Only three need naming,
because the rest derive from one of them:

| variable | set to | default would be |
|---|---|---|
| `GLPI_CONFIG_DIR` | `/var/lib/glpi/config` | `<source>/config` |
| `GLPI_VAR_DIR` | `/var/lib/glpi/files` | `<source>/files` |
| `GLPI_MARKETPLACE_DIR` | `/var/lib/glpi/marketplace` | `<source>/marketplace` |

Fifteen further constants hang off `GLPI_VAR_DIR`: `GLPI_DOC_DIR` is that
directory itself, and fourteen subdirectories of it — `_cache`, `_cron`,
`_graphs`, `_locales`, `_lock`, `_log`, `_pictures`, `_plugins`, `_rss`,
`_sessions`, `_tmp`, `_uploads`, `_inventories`, `_themes`. GLPI creates them
itself (`SystemConfigurator.php:297`), so none needs declaring — and it does so
at **install** time, not lazily on first web request: `bin/console` boots
through the same `SystemConfigurator` as every other entry point, so
`database:install` in §2.5 is what brings them into being. `glpi.nix`'s
`tmpfiles` rules create only the four roots above; the rest are GLPI's.

Because every GLPI process — php-fpm, the cron unit, the console — runs as
`glpi`, it does not matter which one gets there first. The ownership cannot
diverge, which is why lazy creation is safe here and would not be under mixed
users.

`/var/lib/glpi/config` is where `config_db.php` lands, holding the database
connection — and beside it `glpicrypt.key`, **GLPI's encryption key**
(`src/GLPIKey.php:101`; a legacy `glpi.key` is still read as a fallback). Two
files, not one: the key is not inside `config_db.php`, which is worth knowing
because the key is what decrypts the encrypted columns *inside* a dump. That is
why the tree is `0750 glpi:glpi` and why a backup of it is as sensitive as the
database dump.

All three variables go to php-fpm, the cron unit **and** the `glpi-console`
wrapper, and all three must agree. A console run with different paths writes a
`config_db.php` the web process cannot see.

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
  account uses MariaDB's `unix_socket` plugin and has no password (§0, *What
  "over a unix socket" ... actually mean*, for what that implies), but GLPI
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

**Why this stays optional rather than part of the install:** GLPI classes it
that way itself. `DbTimezones` passes `true` as `AbstractRequirement`'s third
constructor argument, `$optional`, so a server without timezone data is not a
deficient one — which is why `system:check_requirements` reported *23 OK, 0
errors, 0 warnings* here before any of this was loaded. The step does belong
after §2.5's schema creation, though, because `database:enable_timezones`
checks the schema for columns still using the deprecated `datetime` type.

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
# the application itself. -p MATTERS: without it `system:status` redacts the
# schema versions AND hardcodes `uptodate` to false, so the check never runs
# (StatusChecker.php:181, :188). Output is JSON, not a summary line.
sudo -u glpi glpi-console system:status -p
sudo -u glpi glpi-console system:check_requirements # expect 0 errors, 0 warnings

# the loopback listener
curl -s -o /dev/null -w '%{http_code}\n' http://localhost:8080/        # 200

# the LAN listener, with hostname verification ON
curl -s -o /dev/null -w '%{http_code} verify=%{ssl_verify_result}\n' \
  --cacert /var/lib/glpi-tls/cert.pem https://nixos.local/             # 200 verify=0

# the inventory endpoint
curl -s -o /dev/null -w '%{http_code}\n' http://localhost:8080/Inventory
```

Read the individual services out of that JSON rather than the `glpi` key.
`glpi` is **not** GLPI's own health: `getServiceStatus('all')` overwrites it
with a roll-up over every service (`StatusChecker.php:149`), and
`calculateGlobalStatus` returns `PROBLEM` if any one of them is (`:118-128`).
A single stuck crontask therefore reads as `glpi: PROBLEM` — see §8.3, where
that is exactly what happened. Expect `db: OK`, `filesystem: OK`,
`database_version: OK`, and `NO_DATA` for every service this install does not
configure.

That last one should return **405, not 403**. 405 means the enabled-check
passed and the endpoint is live — it is simply refusing a GET, which is the
correct answer for an endpoint that only accepts POST. A 403 means step 2.7
did not take. **All four verified here.**

One more check worth running deliberately:

```sh
curl -s -o /dev/null -w '%{http_code} verify=%{ssl_verify_result}\n' \
  --cacert /var/lib/glpi-tls/cert.pem \
  "https://$(avahi-resolve -n -4 nixos.local | awk '{print $2}')/"
```

This should **fail**, with `verify=1`. The address is not in the certificate's
SAN, and that failure is the proof hostname verification is genuinely enforced
rather than encryption alone.

**The address is derived, not written out, and that is the whole point of the
test.** A hardcoded `https://192.168.2.3/` also fails — but after a DHCP lease
change it fails because *nothing is at that address*, not because the address
is absent from the SAN. Same `curl` exit, a different reason, and the check
silently stops testing what it claims to while still reading as a pass. Taking
the address from mDNS keeps the two causes from being confused: if the name
resolves and the connection still refuses to verify, the SAN is what refused
it. Watch for `verify=1` specifically rather than only a non-200 — see §8.5.

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
`GLPI_VAR_DIR` and `GLPI_MARKETPLACE_DIR` (§0 for what each covers) must agree
across php-fpm, the cron unit and the console — a console run with different paths writes a
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

`/var/lib/glpi/config/config_db.php` holds the database credentials, and
`glpicrypt.key` beside it holds **the encryption key** (`src/GLPIKey.php:101`)
— which is why that directory is 0750 and why a backup of it is as sensitive as
the database itself. Take the dump and this tree with the *same* timestamp:
restore a dump against a different key and the encrypted columns come back as
garbage with no error. `/var/lib/glpi/files` holds every
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
| 3306/tcp | `127.0.0.1` and `[::1]` | nothing, and it needs nothing | MariaDB. Both loopbacks, via `settings.mysqld.bind-address = "127.0.0.1,::1"` in `glpi.nix`; nixpkgs sets none, so MariaDB's own default binds every interface. GLPI uses the socket, not this |

### Files

| path | mode | what |
|---|---|---|
| `/etc/nixos/glpi.nix` | — | package + module: PHP env, nginx, MariaDB, TLS, cron |
| `/etc/nixos/glpi-agent.nix` | — | the agent module, importable on any NixOS host |
| `/var/lib/glpi` | 0750 glpi:glpi | all mutable state |
| `/var/lib/glpi/config` | 0750 glpi:glpi | `config_db.php` — credentials; `glpicrypt.key` — encryption key |
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

## 8. Verification pass — 2026-11-24

Everything in §2.8 and §4 re-run against the running system three weeks after
it was built, plus the items *Not proven* below had left open. The read-only
checks were executed and their output read; the changes are marked by who ran
them.

### What still holds

| check | expected | result |
|---|---|---|
| `http://localhost:8080/` | 200 | 200 |
| `https://nixos.local/` with `--cacert` | 200 verify=0 | 200 verify=0 |
| `http://localhost:8080/Inventory` | 405, not 403 | 405 |
| `https://192.168.2.3/` | must **fail** | 000 verify=1 |
| MariaDB listeners | both loopbacks | `127.0.0.1:3306`, `[::1]:3306` |
| reachable on `192.168.2.3` | 443 only | 443 open; 8080, 3306, 3200 unreachable |
| `/var/lib/glpi`, `/var/lib/glpi/config` | 0750 | 0750 |
| the loop closes | one agent, recent | `nixos-2026-10-31-08-36-00`, 15 h, not stale |
| timers | cron minutely, agent daily | `glpi-cron` ran 46 s prior; `glpi-agent` next 00:03 |

**Certificate expiry — the open item in *Not proven*, now closed.** Read off
the live TLS handshake rather than with `openssl`, which is still absent from
this system outside the generating unit: Python's `ssl` module returns the peer
certificate without it.

```
CN=nixos.local    SAN: DNS:nixos.local, DNS:nixos    (no IP SAN, as designed)
notBefore Oct 31 07:07:12 2026 GMT
notAfter  Oct 28 07:07:12 2036 GMT     -> 3625 days remaining
```

This file used to record `notAfter=Oct 28 06:46:41 2036 GMT`. The live
certificate is twenty minutes later, so it was **reissued once** after that
note was written — the event §5 warns invalidates every copy already
distributed to clients. Nothing was lost, because the only client is this host,
but the recorded value described a certificate that no longer exists. Re-read
it from the handshake rather than trusting a transcribed figure.

### 8.1 nginx was IPv4-only while mDNS advertised both families — **fixed**

The one real defect this pass found, and the one a same-machine test cannot
find on principle.

`services.avahi.publish.addresses = true` publishes an **A and an AAAA** for
this host, so `nixos.local` resolves to either. nginx listened on
`0.0.0.0:443` and nothing else. The server got away with it only because
`configuration.nix` sets `nssmdns4 = true` and **not** `nssmdns6`, so its own
resolver never returns the AAAA — the one host in the setup configured not to
ask the question.

A client that does mDNS natively and dual-stack gets the AAAA and hits a
connection refused against a name that resolved perfectly. **That is Windows
and macOS**, which is to say precisely the two platforms §3.5 and §3.6 could
not test.

Demonstrated before the fix, forcing the AAAA while leaving the hostname
intact so certificate verification still applies:

```sh
V6=$(avahi-resolve -n -6 nixos.local | awk '{print $2}')
curl -s -o /dev/null -w '%{http_code} verify=%{ssl_verify_result}\n' \
  --cacert /var/lib/glpi-tls/cert.pem \
  --resolve "nixos.local:443:[$V6]" https://nixos.local/
```

`000 verify=0` against `200 verify=0` for the same name over v4 — the name
resolves, the certificate would verify, and nothing is listening.

The fix is a second entry in the vhost's explicit `listen` list in `glpi.nix`:

```nix
++ lib.optionals cfg.lan.enable [
  { addr = "0.0.0.0"; port = cfg.lan.port; ssl = true; }
  { addr = "[::]";    port = cfg.lan.port; ssl = true; }
];
```

`listenString` in nginx/default.nix interpolates `addr` verbatim, so this
emits `listen [::]:443 ssl;`. It costs no certificate work — the cert carries
DNS names and no IP SAN, so it covers either family — and
`networking.firewall.allowedTCPPorts` already applies to both.

Confirmed after the rebuild: `ss -ltn` shows both `0.0.0.0:443` and
`[::]:443`, and the `--resolve` probe above returns `200 verify=0` over either
family. Generation
`951kw3f4kvs5i9mzvigp135gnym8fjcl-nixos-system-nixos-26.05.20261012.7993f34`.

The alternative, suppressing AAAA publication in avahi, closes the same
mismatch and costs this host v6 mDNS for everything else. The listener is the
cheaper half.

### 8.2 Backup — **taken once 2026-11-24; not automated, and on the same disk**

There was no dump anywhere on this system, no systemd unit, nothing in
`$HOME`. One has now been taken:

```
glpi-2026-11-24.sql        6 927 429 B   449 CREATE TABLEs
glpi-state-2026-11-24.tar.gz  2 725 598 B
glpi-tls-2026-11-24.tar.gz        4 065 B
```

The table count is the check that the `--databases glpi glpi_inv` correction
below actually took: `live` counts both schemas, so 449 = 449 can only hold if
both are in the file. A dump of `glpi` alone would have come in one short.

**What is still open.** It ran by hand, so there is no second one, and
`/var/lib/glpi-backup` sits on `/dev/nvme0n1p5` — the same filesystem as
`/var/lib/mysql` and `/var/lib/glpi`. It survives a bad migration, a dropped
table and a botched upgrade; it does not survive the disk. Copying it off is a
separate decision from taking it.

**What is actually at risk, which is narrower than "the database".** Every
agent re-reports daily, so a lost `glpi` schema followed by a fresh
`database:install` and one inventory round brings back the computers, their
hardware and their software. The inventory is the bulk of the rows and it is
self-healing. What does *not* come back:

- **The OAuth client.** The id and secret in `~/.config/glpi-inv/env` identify
  a row GLPI keeps in its own database (`oauth-setup.md`). Lose the database
  and the env file names a client that does not exist; every API call this
  dashboard makes fails. That file is not a backup of the client.
- **The settings GLPI needs before it will answer at all** — `enable_api`,
  `enable_hlapi`, `enable_api_login_credentials`, `url_base`, `url_base_api`,
  and §2.7's native-inventory switch. All off by default, all in the database.
  Recoverable from these files, but that is an hour of re-reading, not a
  restore.
- **The read-only service-account profile** — hand-built rights, in the
  database. See `gl-rights.md`.
- **`glpi_logs`** — field-level change history, which `gl-sql-plan.md` names as
  one of the two things no GLPI API exposes. No inventory round regenerates
  history; this is the only copy.
- **`glpi_inv.status_samples`** — the minutely time series every history panel
  on the Perf and Status pages plots. Also the only copy.

So the case for this step is **two irreplaceable time series and an hour of
re-setup**, not a lost asset inventory. Worth stating, because the usual
argument for backing up a CMDB does not apply to an install whose data walks
back in by itself.

**Two things overstated when this section was first written.**
`/var/lib/glpi-tls` is cheap to lose *today*: the only agent is this host,
reading the certificate out of that same directory, so regenerating costs a
rebuild and nothing else. §5's warning is about a future with real clients, and
it becomes true the moment §8.1's work pays off and a second one exists.
And `glpicrypt.key` has no independent value — it matters only *for* restoring
a dump.

> **Corrected at source, 2026-11-24.** §0, §5 and §6 used to say
> `config_db.php` holds the database credentials *and* the encryption key. It
> does not: the key is `glpicrypt.key`, a separate file beside it
> (`src/GLPIKey.php:101`, with a legacy `glpi.key` still read as a fallback).
> Both sit in `/var/lib/glpi/config`, so "back up `/var/lib/glpi`, and it is
> 0750 for a reason" was always right — only the attribution was wrong. Noted
> here because it is the reason the dump and the state tree have to be taken
> with the same timestamp, which no longer has to be rediscovered.

**The run.** One script rather than a sequence, because the sequence was tried
and failed: the `install -d` was a separate step, it was skipped, and four
commands then redirected into a directory that did not exist — six cascading
errors from one missing line.

```sh
#!/usr/bin/env bash
set -euo pipefail

TS=$(date +%F)
D=${GLPI_BACKUP_DIR:-/var/lib/glpi-backup}

# 0700 root: the dump carries every stored credential, and the state tree
# carries glpicrypt.key, which decrypts the encrypted columns inside it.
install -d -m 0700 -o root -g root "$D"

# BOTH databases. `glpi_inv` is a separate schema this app creates for its own
# history; a dump of `glpi` alone silently omits it.
mariadb-dump --single-transaction --databases glpi glpi_inv > "$D/glpi-$TS.sql"

tar czf "$D/glpi-state-$TS.tar.gz" -C /var/lib glpi
tar czf "$D/glpi-tls-$TS.tar.gz"   -C /var/lib glpi-tls
chmod 0600 "$D"/*

# Verified, not assumed: a truncated dump is still a plausible-looking file.
tail -1 "$D/glpi-$TS.sql"
in_dump=$(grep -c '^CREATE TABLE' "$D/glpi-$TS.sql")
live=$(mariadb -N -e "SELECT COUNT(*) FROM information_schema.tables \
    WHERE table_schema IN ('glpi','glpi_inv')")
[ "$in_dump" = "$live" ] && echo "OK - counts agree" \
  || { echo "MISMATCH - do not trust this dump" >&2; exit 1; }
```

`set -euo pipefail` so it stops at the first failure instead of running four
more commands that cannot work. Re-running on the same day overwrites rather
than accumulating.

Confirmed on the first run: `-- Dump completed on 2026-11-24 16:03:58`, and
`dump=449 live=449`.

**Follow-on, not done:** wrap it in a `systemd.services.glpi-backup` and
`.timer` with `Persistent = true` — the same reason `glpi-cron.timer` sets it,
in §5: without it a laptop asleep at the scheduled hour never runs the job.

### 8.3 The console checks — **all three run; the one fault found is cleared**

```sh
sudo -u glpi glpi-console system:status -p              # NOTE the -p
sudo -u glpi glpi-console system:check_requirements
sudo -u glpi glpi-console database:check_schema_integrity
```

Use the wrapper (`/run/current-system/sw/bin/glpi-console`), never
`php bin/console` — §5 for why.

**`-p` is not cosmetic, and this is the trap.** Without it, `system:status`
redacts `defined` and `installed` *and* hardcodes `uptodate` to `false` —
`StatusChecker.php:181` is `$public_only ? false : Update::isDbUpToDate()`, so
the schema check never runs. Line 188 likewise hardcodes the status to
`NO_DATA`. A sanitised run therefore reports a database that looks out of date
and has not been looked at. With `-p`:

```
"database_version": { "defined":  "12.0.1@5768fcc43eb6712d0bc124e36b6be3f1e5e506fb",
                      "installed":"12.0.1@5768fcc43eb6712d0bc124e36b6be3f1e5e506fb",
                      "uptodate": true, "status": "OK" }
```

Defined equals installed, so the 11.0.12 → 12.0.1 migration completed.

> **Corrected at source, 2026-11-24.** §2.8 used to say to expect
> `glpi: OK, db: OK` from `system:status`. GLPI 12 prints **JSON**, not that
> line, and the `glpi` key cannot be read as GLPI's own health:
> `getServiceStatus('all')` **overwrites** `$status['glpi']['status']` with a
> roll-up over every service (`StatusChecker.php:149`), and
> `calculateGlobalStatus` returns `PROBLEM` if any one of them is
> (`:118-128`). `glpi` is a global summary. §2.8 now says so, and says to pass
> `-p`.

**The one fault.** `crontasks` is `PROBLEM`:

```
"stuck": ["queuedwebhook"]
"status_msg": "RUNNING: 1, STUCK: 1, ERRORED: 0, TOTAL: 50"
```

Proof that `glpi: PROBLEM` is only this, counted twice: it stayed `PROBLEM`
across both runs while `database_version` moved from `NO_DATA` to `OK`.

`cronQueuedWebhook()` is in `src/QueuedWebhook.php:616`, so the itemtype is
`QueuedWebhook`:

```sh
sudo -u glpi glpi-console task:unlock -t QueuedWebhook::queuedwebhook
```

Scoped to the one task rather than `--all` (`UnlockCommand.php:57`; the default
staleness threshold is 1800 s). Run 2026-11-24:

```
Task "QueuedWebhook::queuedwebhook" unlocked.
Number of tasks unlocked: 1.
glpi: OK
crontasks: OK   RUNNING: 0, STUCK: 0, ERRORED: 0, TOTAL: 50
stuck: []
```

Which closes the loop on the roll-up: `glpi` went to `OK` the moment
`crontasks` did, with nothing else in the document changing. `RUNNING` also
fell 1 → 0, so the stuck task was the one being counted as running.

**Why it was worth clearing, given nothing here uses webhooks** — `plugins`
is `NO_DATA` and no function in use was blocked. Because it pinned
`system:status` to `PROBLEM` permanently, and that costs the signal: the next
real fault would not have changed the output. A monitor that is always red is
not a monitor.

Everything else was `OK` or `NO_DATA` — no LDAP, IMAP, CAS, mail collectors,
replicas or plugins are configured on this install, so `NO_DATA` is the
correct answer rather than a gap.

`system:check_requirements` passed every `[REQUIRED]` and `[SECURITY]` item,
with two `[SKIPPED]`: SELinux, not applicable on NixOS, and the session
security configuration, which the CLI cannot evaluate without a web request.

`database:check_schema_integrity` is the one of the three whose exit code
carries meaning — seven codes, of which `2` is found differences and `3` is
requires a database update (`CheckSchemaIntegrityCommand.php:54-90`). It
returned:

```
Database schema is OK.
exit=0
```

So the 11.0.12 → 12.0.1 move left no drift in the table definitions, which is
a stronger statement than `uptodate: true` on its own: that one compares a
recorded version string, this one compares the schema against the empty-SQL
GLPI ships for its own version.

### 8.4 Default passwords — **checked; three are still shipped, left in place by decision 2026-11-24**

Not a matter of logging in and looking. GLPI ships no default-password console
command — checked against the 12.0.1 command list, where the `security:*`
commands are `change_key`, `change_oauth_key`, `session:list` and
`session:revoke`. The check is exact anyway, because
`install/empty_data.php` seeds precisely these:

| account | shipped password |
|---|---|
| `glpi` | `glpi` |
| `post-only` | `postonly` |
| `tech` | `tech` |
| `normal` | `normal` |
| `glpi-system` | empty — a system account, not a login |

and `Auth::getPasswordHash()` is `password_hash($p, PASSWORD_DEFAULT)`, checked
by `password_verify()` (`src/Auth.php:426` and `:398`). So PHP's own verifier
against the stored hash settles it:

```sh
PHP=$(systemctl cat phpfpm-glpi | grep -oE '/nix/store/[^ ]*php[^/ ]*/bin/php' | head -1)
sudo mariadb -N -B glpi -e \
  "SELECT name,password,is_active FROM glpi_users WHERE name IN ('glpi','post-only','tech','normal')" |
while IFS=$'\t' read -r name hash active; do
  case $name in glpi) p=glpi;; post-only) p=postonly;; tech) p=tech;; normal) p=normal;; esac
  echo "$name: $("$PHP" -r 'echo password_verify($argv[1],$argv[2])?"DEFAULT":"changed";' "$p" "$hash") (is_active=$active)"
done
```

`$PHP` is derived from the running unit rather than written out, so it does not
go stale at the next rebuild.

**The result, 2026-11-24:**

```
glpi:      changed (is_active=1)
normal:    DEFAULT (is_active=1)
post-only: DEFAULT (is_active=1)
tech:      DEFAULT (is_active=1)
```

So §2.6 took for `glpi` and for none of the others. All three are active, on a
server whose `0.0.0.0:443` listener is reachable from the LAN, and the same
seed sets `use_password_security => 0`, so nothing ever forced a change.
`tech` is the one that carries weight: a technician login is write access to
the asset database, not a read.

**Left as it is, deliberately, on 2026-11-24.** Recorded here rather than
quietly dropped, because the point of this section is that nothing reads as
more done than it is.

**Nothing depends on them**, which is worth knowing for whenever the decision
is revisited: this dashboard signs in as a separate user `inv`
(`GLPI_USERNAME`, `grant_type=password`), and the inventory endpoint takes
unauthenticated POSTs by design — that is what §2.7 enables. So all three can
go without breaking anything:

```sh
for u in tech normal post-only; do sudo -u glpi glpi-console user:disable "$u"; done
```

`user:disable` takes a bare login as a positional argument
(`AbstractUserCommand.php:49`) and only sets `is_active = 0`
(`DisableCommand.php:56-60`), so `user:enable` reverses it — which is why it is
the one to reach for rather than `user:delete`. Note that disabling leaves the
hash alone, so the check above will still print `DEFAULT` afterwards; the
column that changes is `is_active`.

### 8.5 The DHCP lease — **deliberately not pinned, 2026-11-24**

Closed as a decision, not as an open item. The analysis below is why: once
§2.8 stopped hardcoding the address, a reservation protected nothing but
documentation literals. Recorded in full so the decision can be revisited
without redoing the reasoning.


| | |
|---|---|
| address to reserve | `192.168.2.3` |
| MAC | `f8:5e:a0:1e:91:23` |
| interface | `wlp0s20f3` — **wireless** |
| router / DHCP / DNS | `192.168.2.254` |
| lease | 14400 s |
| local domain | `home` |

The router model could not be identified from the network: port 80 is open and
`/` returns a bare 404 with no `Server` header. Gateway MAC OUI `7c:39:53`.

Two things would defeat the reservation silently. **It is the wireless MAC** —
plug in ethernet and the lease is keyed to a different one. And **MAC
randomization** would break it outright; not in effect here, since
`f8:5e:a0:1e:91:23` has the locally-administered bit clear and is therefore the
burned-in address rather than a per-connection random one.

If it is ever set, confirm with:

```sh
sudo nmcli device disconnect wlp0s20f3 && sudo nmcli device connect wlp0s20f3
ip -4 -o addr show wlp0s20f3 | grep 192.168.2.3
avahi-resolve -n -4 nixos.local
```

Note that a reservation is **not** detectable from this side — the client still
receives an ordinary lease and still reads `dynamic` in `ip addr`, and a
release/renew returning the same address proves nothing, because a DHCP server
normally re-offers the previous lease anyway. The router's own interface is the
only confirmation.

### What actually happens if the lease moves

Measured rather than assumed, because the answer decides whether this step is
worth doing at all.

**avahi tracks the interface; it does not store a value.** The proof is already
running here: this interface carries five rotating IPv6 privacy addresses, and
two `avahi-resolve -n -6 nixos.local` queries a second apart returned
*different* ones, each the currently live address. A v4 lease change rides the
same mechanism.

**No configuration holds the literal address.** `grep` over
`/etc/nixos/*.nix`, the dashboard's env file and the agent's config file
returns zero hits for `192.168.2.3`.

| | why it survives a change |
|---|---|
| nginx | listens on `0.0.0.0` and `[::]` — every address, so the new one is picked up with no rebind. Pinned to `192.168.2.3` it would instead **fail to start** |
| TLS | no IP SAN, so the certificate matches `nixos.local` at any address |
| glpi-agent | `server = https://nixos.local/` — a name |
| the dashboard → GLPI | `localhost:8080` |
| the dashboard → MariaDB | `localhost`, unix socket |
| firewall | opens a port, not an address |

mDNS address records are short-lived and avahi sends a goodbye plus a fresh
announcement on change, so a client can briefly hold the old address. The
agent runs daily, which makes that window irrelevant to it.

**What a move does break is one test, and it breaks it invisibly.** §2.8's
deliberate-failure probe used to read `https://192.168.2.3/`. It exists to
prove the certificate's SAN is enforced, so it has to fail *for that reason*.
After a lease change it fails because nothing is at the address — same exit,
different cause, and a reader ticks it off having proved nothing. §2.8 now
derives the address from mDNS and watches for `verify=1`, which fixes the test
regardless of whether the lease is ever pinned.

Beyond that it is prose: six occurrences in this file, five in `gl-net.md`.

So this step protects eleven lines of documentation and, before the §2.8
change, one self-invalidating test. Belt-and-braces rather than a fix, for the
reason §1 gives: the certificate carries no IP SAN, so a moved lease breaks no
TLS and mDNS re-announces by itself. It saves confusion, not an outage — and
with §2.8 fixed, less of even that.

**Hence the decision not to pin it.** What is left to go stale is prose, and
prose that is wrong about an address is visible on the next read; a test that is
wrong about *why* it passed is not, which is why that half was worth fixing and
this half was not. If a second client ever makes the address worth quoting to
someone else, the values above are what to enter.

---

## Not proven

Stated plainly so nothing here reads as more tested than it is.

- **No second client.** The only agent is the server itself. §8.1 was its
  prerequisite and is now fixed; the IPv6 listener it adds is what a Windows or
  macOS client would otherwise have fallen over. Every claim about
  the client-server path is true for a host that resolves its own mDNS name
  and goes out over the network to reach itself — which exercises the whole
  chain, but is not a genuinely separate machine.
- **Windows (3.5) and macOS (3.6) are untested**, written from upstream
  documentation with no such host available. 3.6 flags one path it could not
  confirm at all.
- **The DHCP lease is not pinned** in the router — no longer an open item but
  a decision, taken 2026-11-24; see §8.5 for what a lease change actually costs,
  measured rather than assumed, and for the values to enter if it is revisited.
  Note that a reservation is not verifiable from the client in any case.
- **Certificate expiry** — no longer open. Re-read from the live handshake on
  2026-11-24; see §8. It expires `Oct 28 07:07:12 2036 GMT`, twenty minutes
  later than the figure this list used to carry, which means the certificate
  was reissued once since.

## See also

| file | what |
|---|---|
| `gl-net.md` | the reasoning behind every decision above, and the eleven traps in full |
| `gl-upgrade.md` | what a GLPI upgrade breaks downstream, and in what order |
| `gl-rights.md` | profile rights for a read-only API service account |
| `gl-db-api.md` | what defines GLPI's API surface — code, not database |
| `gl-sql-plan.md` | the two things no GLPI API exposes, and what reaching them costs |
