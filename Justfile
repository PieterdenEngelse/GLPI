# The one definition of every command for this project, as in RERAG and
# nixdash. If a command is worth running twice, it belongs here.
#
# `just` lives in the dev shell, so from outside:
#   nix develop --command just <recipe>

default: build

# Backend only. The frontend is wasm and is built by dx, never by a
# host-target cargo build.
#
# Build the backend.
build:
    cargo build

build-release:
    cargo build --release

# Dioxus dev server with hot reload, for working on the UI.
dev-frontend: css-build
    dx serve --package glpi-inv-frontend

# Installs the npm tree first if absent - 34 packages, seconds - so a fresh
# clone is not left guessing why the CSS did not change.
#
# Regenerate the Tailwind/daisyUI stylesheet.
css-build:
    cd frontend/fro && [ -d node_modules ] || npm install --no-audit --no-fund
    cd frontend/fro && npm run css:build

# Depends on css-build deliberately: class names live in the .rs files, so a
# component edit can invalidate output.css without touching any stylesheet,
# and a bundle shipping yesterday's CSS fails silently - it looks like a
# styling mistake rather than a stale artefact.
#
# Build the wasm bundle.
build-frontend: css-build
    dx build --package glpi-inv-frontend

build-frontend-release: css-build
    dx build --release --package glpi-inv-frontend

# Everything, launching nothing.
build-all: build-release build-frontend-release

# Run the backend. Needs GLPI_CLIENT_ID and GLPI_CLIENT_SECRET in the
# environment - it refuses to start without them rather than failing later
# with a confusing 401 from GLPI.
#
# Run the backend. Needs credentials in the environment.
run:
    cargo run

# Probe GLPI's API directly, bypassing the backend and the browser. The fast
# answer to "is it GLPI or is it us".
#
# GET a GLPI API path and print the JSON.
probe path="Inventory/Agent":
    cargo run -p glpi-cli -- {{path}}

# Regenerate the right -> (tab, group) map the Rights page groups by.
#
# Needs the glpi user, so it is the one recipe here you cannot run as yourself:
# GLPI's getRightsForForm() reaches $DB->listFields(), so it needs the full
# bootstrap, which reads config/ - glpi-owned. See the script's header for why
# parsing the PHP instead does not work.
#
# Re-run after a GLPI upgrade. If you forget, the Rights page says so rather
# than mis-filing anything: rights the map does not cover are listed under
# "Not on the profile form", and map entries GLPI no longer has are flagged
# stale. An unchanged GLPI produces no diff.
#
# Both store paths are read out of the glpi-console wrapper rather than pinned,
# so this keeps working across GLPI rebuilds.

# Regenerate the Rights page grouping. Needs the glpi user.
right-taxonomy:
    #!/usr/bin/env bash
    set -euo pipefail
    src=$(grep -oE '/nix/store/[a-z0-9]+-glpi-[0-9.]+' /run/current-system/sw/bin/glpi-console | head -1)
    php=$(grep -oE '/nix/store/[a-z0-9]+-php-with-extensions-[0-9.]+/bin/php' /run/current-system/sw/bin/glpi-console | head -1)
    [ -n "$src" ] && [ -n "$php" ] || { echo "could not locate GLPI or php from the glpi-console wrapper" >&2; exit 1; }
    out=backend/src/glpi/right_taxonomy.json
    # Via a temp file, with the trap armed BEFORE anything can fail: a failed
    # dump must neither truncate the committed map nor leave a stale .tmp that
    # the next run would mistake for output.
    trap 'rm -f "$out.tmp"' EXIT
    # display_errors=stderr matters: PHP writes fatals to STDOUT by default, and
    # stdout here is the output file - so an error would land silently inside
    # right_taxonomy.json.tmp instead of on the terminal.
    sudo -u glpi env GLPI_SRC="$src" GLPI_CONFIG_DIR=/var/lib/glpi/config GLPI_VAR_DIR=/var/lib/glpi/files GLPI_MARKETPLACE_DIR=/var/lib/glpi/marketplace "$php" -d display_errors=stderr -d error_reporting=E_ALL scripts/dump-glpi-right-taxonomy.php > "$out.tmp"
    [ -s "$out.tmp" ] || { echo "dump produced nothing" >&2; exit 1; }
    mv "$out.tmp" "$out"
    echo "wrote $out ($(grep -c '"right"' "$out") entries)"

fmt:
    cargo fmt --all

# Fast checks, for docs and comment changes.
gate:
    cargo fmt --all -- --check
    cargo test

# The above plus clippy, for Rust code paths. Slower.
gate-rust: gate
    cargo clippy --all-targets -- -D warnings
    # The frontend is NOT covered by the line above. `default-members` is the
    # backend, and the frontend is a wasm target, so a root clippy never
    # compiles it - this recipe reported green while the UI was broken until
    # this line existed. Same reason `cargo test` above only runs backend
    # tests.
    cargo clippy -p glpi-inv-frontend --target wasm32-unknown-unknown -- -D warnings

# Copy the release binary and bundle to ~/.local/lib/glpi-inv, where the
# systemd unit expects them. Deliberately a copy and not a symlink into
# target/: the point is that `just clean` cannot reach what is running.
#
# GLPI_INV_STATIC_DIR in ~/.config/glpi-inv/env must name the web/ directory
# below - without it the binary looks for a bundle beside its own path, finds
# none there, and serves the API with no page.
#
# Install binary + bundle to ~/.local/lib and restart the service.
deploy: build-release build-frontend-release
    mkdir -p ~/.local/lib/glpi-inv
    # Copy beside the target and rename over it, never straight onto it.
    # Writing to the path the running service is executing fails outright
    # with ETXTBSY ("Text file busy"); rename(2) is atomic and the live
    # process keeps its own inode until the restart below swaps it.
    cp target/release/glpi-inv-backend ~/.local/lib/glpi-inv/.backend.new
    mv -f ~/.local/lib/glpi-inv/.backend.new ~/.local/lib/glpi-inv/glpi-inv-backend
    # Same shape for the bundle, for a different reason: ServeDir opens files
    # per request, so deleting the directory first leaves a window in which
    # every page 404s while the service still reports healthy.
    rm -rf ~/.local/lib/glpi-inv/web.new
    cp -r target/dx/glpi-inv-frontend/release/web/public ~/.local/lib/glpi-inv/web.new
    rm -rf ~/.local/lib/glpi-inv/web.old
    if [ -d ~/.local/lib/glpi-inv/web ]; then mv ~/.local/lib/glpi-inv/web ~/.local/lib/glpi-inv/web.old; fi
    mv ~/.local/lib/glpi-inv/web.new ~/.local/lib/glpi-inv/web
    rm -rf ~/.local/lib/glpi-inv/web.old
    systemctl --user restart glpi-inv.service

clean:
    cargo clean
    rm -rf frontend/fro/dist
