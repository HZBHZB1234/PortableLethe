# PortableLethe Hub

Tauri 2 desktop Hub for the Portable Lethe private-server suite. The core is
pure Rust; no C# or Python code/runtime is embedded. `dotnet publish` is only
used as an external toolchain by the (optional) source-build channel.

## Layout

```
src/            React UI (Dashboard/Versions/Instances/Config/Resources/ClientPackage/Logs)
src/api.ts      typed Tauri command wrappers (UI <-> core contract)
src-tauri/src/  Rust core: commands.rs + core/{version,process,config,storage,package,resources}
patches/        offline-build placeholders (see below)
```

Commands: `hub_status`, `hub_config_schema`, `hub_config_get/set/update`,
`instance_list/create/start/stop/restart/delete`, `version_list/install/switch/rollback`,
`resource_status/sync`, `clientpkg_export`, `log_tail`, `catalog_find`, `request_restart`.
Events: `instance-state`, `sync-progress`, `download-progress`, `log-line`, `restart-requested`.

## Build

```
npm install
npm run build
cd src-tauri && cargo check
```

Hub data root defaults to `%LOCALAPPDATA%/PortableLethe/hub`; override with
`PORTABLE_LETHE_HUB_DIR`.

### Offline build note (this machine)

crates.io is unreachable here, so `Cargo.toml` does not resolve
`zip`/`git2`/`reflink` (pure-Rust equivalents: `core/zipfile.rs`,
`core/storage.rs`, `core/version.rs`). `patches/` holds inert placeholders for
wasm32-only transitive crates plus a jobserver-free `cc`. Prepared Cargo home:

```
$env:CARGO_HOME='C:\Users\tester\cargo-home-portableleth'
$env:CARGO_TARGET_DIR='C:\Users\tester\cargo-target-portableleth'
cargo check --offline
```

On a networked machine remove `[patch.crates-io]` to use the real crates.

## Status

Implemented: prebuilt release download + SHA256 + blob-backed install + junction
switch/rollback; process spawn/health/backoff restart; schema-driven config with
atomic bootstrap and Windows DPAPI; content-addressed blobs/hardlinks/port
allocation; client package export with binary guard; Addressables catalog v2
parser; unityfs static data extraction with `.txt`→`.json` normalization;
modfiles ETag sync.

Stubbed/planned: bundle diff sync (`resource_sync("bundles")` returns an
explicit error), source-build channel (portable dotnet SDK + `dotnet publish`),
full `--chains` expansion.
