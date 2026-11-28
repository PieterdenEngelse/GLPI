# Three version pins that must move together

`dx`, the `dioxus` crate and `wasm-bindgen-cli` are mutually version-locked,
and getting this wrong cost real time:

1. **`dioxus` is pinned with `=0.7.9`**, not `"0.7"`. A caret range resolves
   to the newest 0.7.x and `dx` then refuses outright: *"dx and dioxus
   versions are incompatible: dx 0.7.9, dioxus 0.7.10"*.
2. **`wasm-bindgen-cli` is built at 0.2.130 in `flake.nix`**, because nixpkgs
   ships 0.2.121 and `dx` checks the CLI against the crate. Pinning the crate
   *down* is not possible: `js-sys 0.3.107` requires wasm-bindgen 0.2.130,
   `js-sys` arrives via `chrono` in the backend, and cargo resolves one
   version per workspace.
3. It uses **`pkgs.buildWasmBindgenCli`**, not `.override { version = ...; }`.
   The nixpkgs package is `wasm-bindgen-cli_0_2_121` and its function takes
   only `{ buildWasmBindgenCli, fetchCrate, rustPlatform }` — passing
   `version` fails with *"called with unexpected argument"*.

Bumping `dioxus-cli` in nixpkgs means revisiting all three.

Also: `fetchCrate` hashes the **unpacked** source, so a hash from
`nix store prefetch-file` on the crates.io download URL does not apply. Use a
placeholder and read the real hash from the mismatch.

---

Part of the guidance in [CLAUDE.md](../../CLAUDE.md).
