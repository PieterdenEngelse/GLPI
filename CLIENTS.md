# Pointing clients at this GLPI server

This file used to carry the client-side procedure. It now lives in one place
instead of two: **`gl-client-serv.md`, section 3** — the setup manual in this
directory. Two homes for one procedure is how they drift, and this one had
already started to.

Kept as a pointer rather than deleted, because `gl-net.md` and older notes
link here by name.

| you want | go to |
|---|---|
| the certificate off the server, and why not `--no-ssl-check` | §3.1 |
| the config format and keys, once, for every platform | §3.2 |
| a NixOS client | §3.3 |
| any other Linux | §3.4 |
| Windows — **untested** | §3.5 |
| macOS — **untested**, and one path unconfirmed | §3.6 |
| the server inventorying itself | §3.7 |
| verifying a client actually worked | §4 |

Server: **https://nixos.local/**, self-signed — every client needs
`/var/lib/glpi-tls/cert.pem` as its trust anchor. That file is world-readable
and holds no private key, so it is safe to copy anywhere; `key.pem` never
leaves the server.
