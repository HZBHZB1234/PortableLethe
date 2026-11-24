#requires -Version 7
<#
  Build the non-installed Portable Lethe Hub package (zip, no MSI installer).
  Requires a prior `npm run tauri build` (or `cargo build --release`).
#>
param(
    [string]$Exe = 'C:\Users\tester\cargo-target-portableleth\release\portable-lethe-hub.exe',
    [string]$OutDir = (Join-Path $PSScriptRoot '..\dist'),
    [string]$ServerSchema = 'E:\desktop\work\LimbusDecompile\PortableOpenLethe\config-schema.json'
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$cargoToml = Get-Content -Raw (Join-Path $root 'src-tauri\Cargo.toml')
$version = [regex]::Match($cargoToml, '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
if (-not $version) { $version = '0.1.1' }
$outDir = [IO.Path]::GetFullPath($OutDir)
if (-not (Test-Path $Exe)) { throw "exe not found: $Exe" }
New-Item -ItemType Directory -Force $outDir | Out-Null

$stage = Join-Path $outDir '.portable-stage'
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force $stage | Out-Null

Copy-Item $Exe (Join-Path $stage 'PortableLetheHub.exe') -Force
if (Test-Path $ServerSchema) { Copy-Item $ServerSchema (Join-Path $stage 'config-schema.json') -Force }

$readme = @'
Portable Lethe Hub (no installation required)

Run:  double-click PortableLetheHub.exe
Data: %LOCALAPPDATA%\PortableLethe\hub  (override with PORTABLE_LETHE_HUB_DIR)

This build needs WebView2 Runtime (already present on Windows 11 / modern Edge).
config-schema.json is optional: Hub falls back to built-in fields when absent.

Three decoupled sections (switch any of them independently, no file copies):
  1. Server    - installed server executables. "Set default" only affects NEW
                 instances; existing instances keep their own pinned version.
  2. Data      - resource sets, each a junction root over StaticData / Localize
                 / modfiles. Sources: release zip, official CDN, Lethe CDN, or
                 a locally installed Limbus Company game (auto-detected).
                 Switching sets is O(1), no file copies.
  3. Instances - each deployment binds one server version and one resource set
                 via junctions. Switch either from the instance card; the
                 process restarts automatically.

First-use flow:
  1. Install a server version under "Server" (auto-fetches static data).
  2. Under "Data" import the local game if detected, or sync a source.
  3. Create an instance under "Instances" choosing server + resource set.
  4. Start it and open its admin panel.
  5. ClientPackage -> export a prebuilt launcher bundle for players.
'@
Set-Content -Path (Join-Path $stage 'README.txt') -Value $readme -Encoding UTF8

$zip = Join-Path $outDir ("PortableLetheHub_{0}_portable_win-x64.zip" -f $version)
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip
Remove-Item -Recurse -Force $stage

$item = Get-Item $zip
"PORTABLE=$($item.FullName)"
"SIZE_MB=$([math]::Round($item.Length / 1MB, 1))"
"SHA256=$((Get-FileHash $zip -Algorithm SHA256).Hash.ToLower())"
