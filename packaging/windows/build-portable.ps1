# Build a portable zip: x52-integrity.exe + x52-monitor.exe + README, no installer.
# Run from repo root after `cargo build --release`.

[CmdletBinding()]
param(
    [string]$Version = '0.2.0',
    [string]$ReleaseDir = (Join-Path $PSScriptRoot '..\..\target\release'),
    [string]$OutDir = (Join-Path $PSScriptRoot '..\..\dist')
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$ReleaseDir = (Resolve-Path $ReleaseDir).Path
$gui = Join-Path $ReleaseDir 'x52-integrity.exe'
if (-not (Test-Path $gui)) { throw "Build release first: missing $gui" }

New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
$stage = Join-Path $OutDir "X52-Integrity-$Version-windows-x64-portable"
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Path $stage | Out-Null

Copy-Item $gui $stage
$mon = Join-Path $ReleaseDir 'x52-monitor.exe'
if (Test-Path $mon) { Copy-Item $mon $stage }

$readme = Join-Path $PSScriptRoot '..\..\README.md'
if (Test-Path $readme) { Copy-Item $readme $stage }
$license = Join-Path $PSScriptRoot '..\..\LICENSE'
if (Test-Path $license) { Copy-Item $license $stage }

@'
@echo off
set "X52_INTEGRITY_HOME=%~dp0"
start "" "%~dp0x52-integrity.exe"
'@ | Set-Content (Join-Path $stage 'X52 Integrity.cmd') -Encoding ASCII

@'
X52 Integrity portable build
============================

Double-click "X52 Integrity.cmd" or x52-integrity.exe.

This folder is the data directory (logs, config.json). No Start Menu
shortcuts are created. To install shortcuts, run:

    x52-integrity.exe --install --silent --prefix "%CD%"

'@ | Set-Content (Join-Path $stage 'PORTABLE.txt') -Encoding UTF8

$zip = Join-Path $OutDir "X52-Integrity-$Version-windows-x64-portable.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -Force
Write-Host "Wrote $zip"
