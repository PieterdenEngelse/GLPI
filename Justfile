# The one definition of every command for this project, as in RERAG and
# nixdash. If a command is worth running twice, it belongs here.
#
# `just` lives in the dev shell, so from outside:
#   nix develop --command just <recipe>

default: build

# Backend only. The frontend is wasm and is built by dx, never by a
# host-target cargo build.
build:
    cargo build

build-release:
    cargo build --release

# Dioxus dev server with hot reload, for working on the UI.
dev-frontend:
    dx serve --package glpi-inv-frontend

build-frontend:
    dx build --package glpi-inv-frontend

build-frontend-release:
    dx build --release --package glpi-inv-frontend

# Everything, launching nothing.
build-all: build-release build-frontend-release

# Run the backend. Needs GLPI_CLIENT_ID and GLPI_CLIENT_SECRET in the
# environment - it refuses to start without them rather than failing later
# with a confusing 401 from GLPI.
run:
    cargo run

# Probe GLPI's API directly, bypassing the backend and the browser. The fast
# answer to "is it GLPI or is it us".
probe path="Inventory/Agent":
    cargo run -p glpi-cli -- {{path}}

fmt:
    cargo fmt --all

# Fast checks, for docs and comment changes.
gate:
    cargo fmt --all -- --check
    cargo test

# The above plus clippy, for Rust code paths. Slower.
gate-rust: gate
    cargo clippy --all-targets -- -D warnings

clean:
    cargo clean
    rm -rf frontend/fro/dist
