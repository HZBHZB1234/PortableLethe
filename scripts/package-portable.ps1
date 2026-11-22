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
$outDir = [IO.Path]::GetFullPath($OutDir)
if (-not (Test-Path $Exe)) { throw "exe not found: $Exe" }
New-Item -ItemType Directory -Force $outDir | Out-Null

$stage = Join-Path $outDir '.portable-stage'
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force $stage | Out-Null

Copy-Item $Exe (Join-Path $stage 'PortableLetheHub.exe') -Force
if (Test-Path $ServerSchema) { Copy-Item $ServerSchema (Join-Path $stage 'config-schema.json') -Force }

@'
Portable Lethe Hub (no installation required)

Run:  double-click PortableLetheHub.exe
Data: %LOCALAPPDATA%\PortableLethe\hub  (override with PORTABLE_LETHE_HUB_DIR)

This build needs WebView2 Runtime (already present on Windows 11 / modern Edge).
config-schema.json is optional: Hub falls back to built-in fields when absent.

First-use flow:
  1. Versions  -> install prebuilt server (or build from source).
  2. Resources -> sync modfiles / staticdata / bundles.
  3. Instances -> create + start an instance; open its admin panel with 面板.
  4. ClientPackage -> export a prebuilt launcher bundle for players.
'@ | Set-Content -Path (Join-Path $stage 'README.txt') -Encoding UTF8

$zip = Join-Path $outDir 'PortableLetheHub_0.1.0_portable_win-x64.zip'
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip
Remove-Item -Recurse -Force $stage

$item = Get-Item $zip
"PORTABLE=$($item.FullName)"
"SIZE_MB=$([math]::Round($item.Length / 1MB, 1))"
"SHA256=$((Get-FileHash $zip -Algorithm SHA256).Hash.ToLower())"
