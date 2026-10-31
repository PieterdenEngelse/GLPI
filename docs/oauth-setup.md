# Getting credentials for the GLPI API

The High-Level API uses **OAuth2**, not a user token. The legacy
`/apirest.php` API accepts an App-Token plus a user token, which is simpler,
but it is the deprecated path — see the version list GLPI itself serves at
`/api.php`.

Verified on GLPI 11.0.12:

```
POST /api.php/token   →  400 {"error":"unsupported_grant_type", ...}
```

A real OAuth2 error document, so the endpoint is live and speaking OAuth2.

## Prerequisites, all off by default

```sh
sudo -u glpi glpi-console config:set enable_api 1
sudo -u glpi glpi-console config:set enable_hlapi 1
sudo -u glpi glpi-console config:set enable_api_login_credentials 1
```

Also set the base URLs, or GLPI advertises its own endpoints on the wrong
port — it reported `http://localhost/api.php/v1`, with no `:8080`, until:

```sh
sudo -u glpi glpi-console config:set url_base http://localhost:8080
sudo -u glpi glpi-console config:set url_base_api http://localhost:8080/api.php
```

Any client following the advertised endpoint hits port 80 and fails. This also
affects notification emails.

## Create the client

In GLPI: **Setup → General → API** (OAuth clients). Create one with the
`client_credentials` grant, and copy the id and secret.

Then, with `chmod 600`:

```sh
mkdir -p ~/.config/glpi-inv
cat > ~/.config/glpi-inv/env <<'ENV'
GLPI_URL=http://localhost:8080
GLPI_CLIENT_ID=...
GLPI_CLIENT_SECRET=...
ENV
chmod 600 ~/.config/glpi-inv/env
```

Never put these in the repo or anywhere Nix will copy them: **store paths are
world-readable**, so a secret written by a derivation is exposed locally even
before anything is pushed.

## Check it

```sh
nix develop --command just probe Inventory/Agent
```

`glpi-cli` talks to GLPI directly, so it separates "GLPI is wrong" from
"our backend is wrong" in one command.
