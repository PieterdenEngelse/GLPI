# GLPI on NixOS

A hand-written NixOS package and module for [GLPI](https://glpi-project.org/)
11.0.12 — IT asset management and service desk — plus a module for the GLPI
inventory agent.

nixpkgs has **no `glpi` package and no `services.glpi` module**. It does have
`pkgs.glpi-agent` but **no `services.glpi-agent`**, only
`services.fusionInventory` for the abandoned predecessor. Both modules here
are written from scratch.

Everything was built and run on one machine (NixOS 26.05). `gl-net.md` records
what was verified by executing it and what was not, and documents eight traps
that cost real debugging time — several of which produce a *working build* and
a broken system.

## Layout

| file | what |
|---|---|
| `glpi.nix` | the GLPI package + module: PHP env, nginx, MariaDB, optional LAN TLS, cron |
| `glpi-agent.nix` | the inventory agent module |
| `gl-net.md` | working notes, design reasoning, and the traps |
| `CLIENTS.md` | pointing Windows / macOS / other Linux clients at the server |

## Usage

Drop both files beside your `configuration.nix` and wire them in:

```nix
imports = [
  ./glpi.nix
  ./glpi-agent.nix
];

services.glpi.enable = true;

# Optional: serve to the LAN over HTTPS with a self-signed certificate,
# in addition to the plain-HTTP loopback listener. Opens the firewall port.
services.glpi.lan.enable = true;

# Optional: inventory this machine too.
services.glpi-agent = {
  enable = true;
  server = "https://${config.services.glpi.hostname}/";
  caCertFile = "${config.services.glpi.lan.tlsDir}/cert.pem";
};
```

`services.glpi.hostname` defaults to `${config.networking.hostName}.local`.
A `.local` name needs avahi to **publish**, which is a separate switch from
resolving — avahi answers for other hosts by default and never announces
itself:

```nix
services.avahi = {
  enable = true;
  nssmdns4 = true;
  openFirewall = true;
  publish = { enable = true; addresses = true; workstation = true; };
};
```

Then rebuild, and create the schema **once**:

```sh
sudo nixos-rebuild switch

sudo -u glpi glpi-console database:install \
  --db-name=glpi --db-user=glpi --db-password= --no-interaction
```

`--db-password=` with an **empty value** is required and is not a typo — the
account uses MariaDB's `unix_socket` plugin and has no password, but GLPI
validates the option and rejects a null one. See `gl-net.md`, trap 5.

Native inventory ships **disabled**; the agent endpoint returns 403 until:

```sh
sudo -u glpi glpi-console config:set --context=inventory enabled_inventory 1
```

Then log in at `http://localhost:8080` as `glpi` / `glpi` and **change that
password** — GLPI ships four accounts with published defaults.

## Notable design points

- **GLPI's source stays read-only in `/nix/store`.** It normally writes inside
  its own tree; `SystemConfigurator.php` lets every path constant be set from
  the environment, so state goes to `/var/lib/glpi` instead.
- **The TLS key is generated at runtime, never in a derivation.** Nix store
  paths are world-readable, which is fine for a certificate and unacceptable
  for a private key.
- **Agents verify the self-signed certificate** via `--ca-cert-file` rather
  than skipping the check with `--no-ssl-check`, so a self-signed setup still
  gets authentication and not just encryption.
- **The agent's `environment` and `process` categories are excluded by
  default.** They collect environment variables *with values* (a routine place
  for credentials) and a full process list with command lines — neither tells
  you anything about the hardware or software installed.

## Status and scope

Verified by execution: the module, the loopback install, LAN HTTPS with
certificate verification, and one real agent run creating an asset.

**Not** verified: the Windows and macOS client instructions in `CLIENTS.md`.
They come from upstream documentation, with no such host available to test
them — `CLIENTS.md` marks exactly which parts those are, including one path it
could not confirm.

PHP is taken from the system's nixpkgs by default. If yours predates
**8.4.27**, read the CVE section of `gl-net.md` first: two actively-exploited
interpreter RCEs affect every GLPI version equally, and no GLPI release fixes
them.

## Licence

The modules here: MIT. GLPI itself is GPL-3.0-or-later and is not vendored —
`glpi.nix` fetches the upstream release tarball.
