# Pointing clients at this GLPI server

Server: **https://nixos.local/** — self-signed cert, so every client needs
the certificate as its trust anchor.

Copy `/var/lib/glpi-tls/cert.pem` off this box to each client. It is
world-readable and contains **no private key**, so it is safe to email,
copy to a share, or commit. The key stays at 0600 root and never leaves.

Config keys below were read from the agent's own parser
(`GLPI/Agent/Config.pm`): the file format is `key = value`, keys match the
long option names. `ca-cert-file` and `ca-cert-dir` are mutually exclusive —
setting both is a fatal error (Config.pm:332).

**Prefer `ca-cert-file` over `no-ssl-check`.** With the cert, the agent
verifies the server properly: self-signed TLS then gives both encryption and
authentication. `no-ssl-check` accepts *any* certificate, which protects
against a passive listener and not at all against an active one — on
WPA2-PSK, where every device sharing the passphrase can decrypt its peers,
that difference is the whole point of using TLS here.

## Other NixOS / Linux machines  (tested: module built and run on the server)

Copy `glpi-agent.nix` to the client and import it:

```nix
imports = [ ./glpi-agent.nix ];

services.glpi-agent = {
  enable = true;
  server = "https://nixos.local/";
  caCertFile = "/etc/glpi-agent-ca.pem";   # where you put cert.pem
  tag = "laptop";                          # optional, groups assets
};
```

Place the cert declaratively so it survives a rebuild:

```nix
environment.etc."glpi-agent-ca.pem".source = ./glpi-server-cert.pem;
```

Non-NixOS Linux: install the agent from your distribution or upstream's
AppImage, then write `/etc/glpi-agent/agent.cfg`:

```
server = https://nixos.local/
ca-cert-file = /etc/glpi-agent-ca.pem
tag = laptop
```

Run once with `glpi-agent` to check, then let the packaged timer/cron handle it.

## Windows  (UNTESTED — written from upstream docs, no Windows host here)

MSI from https://github.com/glpi-project/glpi-agent/releases .
Properties verified against upstream's windows-command-line docs; names are
case-sensitive and take no spaces around `=`. From **cmd, not PowerShell**:

```
msiexec /i GLPI-Agent-1.21-x64.msi /quiet ^
  SERVER=https://nixos.local/ ^
  CA_CERT_FILE=C:\ProgramData\GLPI-Agent\glpi-server-cert.pem ^
  TAG=desktop
```

Copy the cert to that path *before* installing. `NO_SSL_CHECK=1` exists but
see the warning above — use the cert.

Note: upstream ships 1.21 while nixpkgs has 1.20. Mixed versions are fine;
the inventory protocol is stable across minor releases.

Windows 10 1703+ resolves `.local` over mDNS natively. If it does not,
either use the server's IP (and reissue the cert with that IP in its SAN) or
add a hosts entry.

## macOS  (UNTESTED, and one detail unconfirmed)

`.pkg` from the same releases page. Upstream documents a standard `.pkg`
plus an unofficial Homebrew route.

The config *keys* are the same as Linux (same parser, same file format):

```
server = https://nixos.local/
ca-cert-file = /etc/glpi-agent-ca.pem
```

**I could not confirm the exact config file path** that the macOS pkg uses —
upstream's macOS page was not reachable at the URLs I tried. After
installing, find it with:

```
sudo find / -name "agent.cfg" -path "*glpi*" 2>/dev/null
```

and write the two lines above into it. Do not take a path from me on faith
here; the rest of this file was verified, this one line was not.

macOS resolves `.local` natively via Bonjour, so the hostname needs no
extra work.

## Verifying a client worked

On the client:

```
glpi-agent --server=https://nixos.local/ --ca-cert-file=<cert> --force
```

`--force` sends even when the agent thinks nothing changed. A TLS failure
here means the cert is wrong or not found; a 403 means inventory is still
disabled on the server; a 404 would mean the server's nginx is blocking
`.php` routes (see the long comment in glpi.nix).

Then on the server: **Assets → Computers**, or
`sudo -u glpi glpi-console system:status`.
