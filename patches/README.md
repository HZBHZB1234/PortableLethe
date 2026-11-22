# Offline build patches

This machine has no crates.io access; only a fixed set of `.crate` archives is
cached. Cargo nevertheless resolves `cfg(target_arch = "wasm32")` dependencies
of `reqwest`, and those archives are missing. `src-tauri/Cargo.toml` patches
those four wasm-only crates with the empty crates here. They are never compiled
for the desktop target; on a networked machine delete the `[patch.crates-io]`
block to use the real crates.
