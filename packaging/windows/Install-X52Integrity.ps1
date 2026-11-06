# X52 Integrity installer for Windows 10 (22H2+) and Windows 11 (24H2–26H2).
#
# Per-user install needs no administrator rights. Shortcuts are created with
# in-process WScript.Shell (no nested `powershell` on PATH — that was the
# original "installer seemed to fail" bug).
#
# Usage:
#   powershell.exe -NoProfile -ExecutionPolicy Bypass -File Install-X52Integrity.ps1
#   powershell.exe -NoProfile -ExecutionPolicy Bypass -File Install-X52Integrity.ps1 -Silent
#   powershell.exe -NoProfile -ExecutionPolicy Bypass -File Install-X52Integrity.ps1 -Machine
#   powershell.exe -NoProfile -ExecutionPolicy Bypass -File Install-X52Integrity.ps1 -Uninstall
#   powershell.exe -NoProfile -ExecutionPolicy Bypass -File Install-X52Integrity.ps1 -Portable -Prefix D:\Apps\X52Integrity
#
# Exit codes: 0 success (shortcut warnings still 0), 1 failure, 2 usage.

[CmdletBinding()]
param(
    [switch]$Silent,
    [switch]$Machine,
    [switch]$Uninstall,
    [switch]$Portable,
    [switch]$NoShortcuts,
    [string]$Prefix,
    [string]$SourceDir
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$ProductName = 'X52 Integrity'
$GuiName = 'x52-integrity.exe'
$MonName = 'x52-monitor.exe'
$TranscriptStarted = $false
$Copied = New-Object System.Collections.Generic.List[string]
$CreatedShortcuts = New-Object System.Collections.Generic.List[string]

function Write-Log {
    param([string]$Message, [string]$Level = 'INFO')
    $line = '{0:HH:mm:ss}  {1,-4}  {2}' -f (Get-Date), $Level, $Message
    if (-not $Silent) {
        if ($Level -eq 'ERR') { Write-Host $line -ForegroundColor Red }
        elseif ($Level -eq 'WARN') { Write-Host $line -ForegroundColor Yellow }
        else { Write-Host $line }
    }
}

function Get-IsAdmin {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    $p = New-Object Security.Principal.WindowsPrincipal($id)
    return $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Request-Admin {
    param([string[]]$PassArgs)
    $self = $MyInvocation.PSCommandPath
    if (-not $self) { $self = $PSCommandPath }
    $argLine = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $self) + $PassArgs
    $p = Start-Process -FilePath (Join-Path $env:WINDIR 'System32\WindowsPowerShell\v1.0\powershell.exe') `
        -Verb RunAs -ArgumentList $argLine -Wait -PassThru
    if ($null -eq $p) { throw 'UAC cancelled' }
    exit $p.ExitCode
}

function Find-Payload {
    param([string]$Hint)
    $candidates = @()
    if ($Hint) { $candidates += $Hint }
    if ($PSScriptRoot) {
        $candidates += $PSScriptRoot
        $candidates += (Join-Path $PSScriptRoot '..\..\target\release')
        $candidates += (Join-Path $PSScriptRoot '..\..\dist')
    }
    $candidates += (Get-Location).Path
    foreach ($c in $candidates) {
        $resolved = $null
        try { $resolved = (Resolve-Path -LiteralPath $c -ErrorAction Stop).Path } catch { continue }
        $gui = Join-Path $resolved $GuiName
        if (Test-Path -LiteralPath $gui) { return $resolved }
    }
    throw "Could not find $GuiName. Build with 'cargo build --release' or pass -SourceDir."
}

function Get-InstallRoot {
    if ($Prefix) { return $Prefix }
    if ($Machine) { return (Join-Path ${env:ProgramFiles} $ProductName) }
    $local = $env:LOCALAPPDATA
    if (-not $local) { $local = Join-Path $env:USERPROFILE 'AppData\Local' }
    return (Join-Path $local 'X52Integrity')
}

function New-Shortcut {
    param([string]$Lnk, [string]$Target, [string]$WorkDir)
    $parent = Split-Path -Parent $Lnk
    if (-not (Test-Path -LiteralPath $parent)) {
        New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }
    # In-process COM — do not spawn powershell.exe (PATH-less nested shells fail).
    $shell = New-Object -ComObject WScript.Shell
    $sc = $shell.CreateShortcut($Lnk)
    $sc.TargetPath = $Target
    $sc.WorkingDirectory = $WorkDir
    $sc.WindowStyle = 1
    $sc.Description = $ProductName
    $sc.IconLocation = "$Target,0"
    $sc.Save()
    [void]$CreatedShortcuts.Add($Lnk)
}

function Copy-FileLogged {
    param([string]$From, [string]$To)
    $dir = Split-Path -Parent $To
    if (-not (Test-Path -LiteralPath $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
    Copy-Item -LiteralPath $From -Destination $To -Force
    Unblock-File -LiteralPath $To -ErrorAction SilentlyContinue
    [void]$Copied.Add($To)
    Write-Log "Copied $To"
}

function Invoke-Rollback {
    Write-Log 'Rolling back' 'WARN'
    foreach ($s in $CreatedShortcuts) {
        Remove-Item -LiteralPath $s -Force -ErrorAction SilentlyContinue
    }
    foreach ($f in $Copied) {
        Remove-Item -LiteralPath $f -Force -ErrorAction SilentlyContinue
    }
}

function Install-Product {
    $src = Find-Payload $SourceDir
    $dest = Get-InstallRoot
    $guiSrc = Join-Path $src $GuiName
    if (-not (Test-Path -LiteralPath $guiSrc)) { throw "missing $guiSrc" }

    Write-Log "Source $src"
    Write-Log "Dest   $dest"
    New-Item -ItemType Directory -Path (Join-Path $dest 'logs') -Force | Out-Null

    Copy-FileLogged $guiSrc (Join-Path $dest $GuiName)
    $monSrc = Join-Path $src $MonName
    if (Test-Path -LiteralPath $monSrc) {
        Copy-FileLogged $monSrc (Join-Path $dest $MonName)
    }
    $legacy = Join-Path $dest 'x52-firmfix.exe'
    Copy-FileLogged (Join-Path $dest $GuiName) $legacy

    $backupSrc = Join-Path $src 'backups\sai075c'
    if (-not (Test-Path -LiteralPath $backupSrc)) {
        $backupSrc = Join-Path $src '..\..\tools\backups\sai075c'
    }
    if (Test-Path -LiteralPath $backupSrc) {
        $backupDst = Join-Path $dest 'backups\sai075c'
        New-Item -ItemType Directory -Path $backupDst -Force | Out-Null
        Get-ChildItem -LiteralPath $backupSrc -File | ForEach-Object {
            Copy-FileLogged $_.FullName (Join-Path $backupDst $_.Name)
        }
    }

    $warnings = @()
    if (-not $Portable -and -not $NoShortcuts) {
        $gui = Join-Path $dest $GuiName
        $desk = Join-Path $env:USERPROFILE "Desktop\$ProductName.lnk"
        $startDir = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\X52 Integrity'
        $start = Join-Path $startDir "$ProductName.lnk"
        try { New-Shortcut $desk $gui $dest; Write-Log "Desktop shortcut $desk" }
        catch { $warnings += "Desktop shortcut: $_"; Write-Log "Desktop shortcut: $_" 'WARN' }
        try { New-Shortcut $start $gui $dest; Write-Log "Start Menu shortcut $start" }
        catch { $warnings += "Start Menu shortcut: $_"; Write-Log "Start Menu shortcut: $_" 'WARN' }
    }

    $manifest = [ordered]@{
        installed_at  = [int][double]::Parse((Get-Date -UFormat %s))
        version       = '0.2.0'
        install_dir   = $dest
        files         = @($Copied)
        shortcuts     = @($CreatedShortcuts)
        shortcuts_ok  = ($warnings.Count -eq 0)
        portable      = [bool]$Portable
        machine       = [bool]$Machine
    }
    $manPath = Join-Path $dest 'install-manifest.json'
    ($manifest | ConvertTo-Json -Depth 5) | Set-Content -LiteralPath $manPath -Encoding UTF8
    [void]$Copied.Add($manPath)

    if ($Portable) {
        $launcher = Join-Path $dest 'X52 Integrity.cmd'
        @"
@echo off
set "X52_INTEGRITY_HOME=%~dp0"
start "" "%~dp0$GuiName"
"@ | Set-Content -LiteralPath $launcher -Encoding ASCII
        Write-Log "Portable launcher $launcher"
    }

    Write-Log "Installed to $dest"
    foreach ($w in $warnings) { Write-Log $w 'WARN' }
}

function Uninstall-Product {
    $dest = Get-InstallRoot
    $manPath = Join-Path $dest 'install-manifest.json'
    $links = @(
        (Join-Path $env:USERPROFILE "Desktop\$ProductName.lnk"),
        (Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\X52 Integrity\X52 Integrity.lnk')
    )
    if (Test-Path -LiteralPath $manPath) {
        try {
            $man = Get-Content -LiteralPath $manPath -Raw | ConvertFrom-Json
            if ($man.shortcuts) { $links += @($man.shortcuts) }
        } catch { }
    }
    foreach ($s in $links) {
        if ($s -and (Test-Path -LiteralPath $s)) {
            Remove-Item -LiteralPath $s -Force -ErrorAction SilentlyContinue
            Write-Log "Removed $s"
        }
        $parent = Split-Path -Parent $s
        if ($parent -and (Test-Path -LiteralPath $parent)) {
            Remove-Item -LiteralPath $parent -Force -ErrorAction SilentlyContinue
        }
    }
    if (Test-Path -LiteralPath $dest) {
        Remove-Item -LiteralPath $dest -Recurse -Force
        Write-Log "Removed $dest"
    } else {
        Write-Log "Nothing to remove at $dest" 'WARN'
    }
}

try {
    try { Unblock-File -LiteralPath $PSCommandPath -ErrorAction SilentlyContinue } catch { }

    if ($Machine -and -not (Get-IsAdmin)) {
        $pass = @()
        if ($Silent) { $pass += '-Silent' }
        $pass += '-Machine'
        if ($Uninstall) { $pass += '-Uninstall' }
        if ($Prefix) { $pass += '-Prefix'; $pass += $Prefix }
        Request-Admin $pass
    }

    $destForLog = Get-InstallRoot
    $logDir = Join-Path $destForLog 'logs'
    New-Item -ItemType Directory -Path $logDir -Force | Out-Null
    $logFile = Join-Path $logDir 'install.log'
    Start-Transcript -Path $logFile -Append | Out-Null
    $TranscriptStarted = $true
    Write-Log "$ProductName installer"
    Write-Log "Windows $([Environment]::OSVersion.Version)  user=$env:USERNAME  admin=$(Get-IsAdmin)  silent=$Silent"

    if ($Uninstall) { Uninstall-Product }
    else { Install-Product }

    Write-Log 'Done'
    if ($TranscriptStarted) { Stop-Transcript | Out-Null }
    exit 0
}
catch {
    Write-Log "$_" 'ERR'
    try { Invoke-Rollback } catch { }
    if ($TranscriptStarted) { try { Stop-Transcript | Out-Null } catch { } }
    if (-not $Silent) {
        Write-Host ''
        Write-Host 'Install failed. See docs/install-windows.md' -ForegroundColor Red
        Write-Host $_.Exception.Message -ForegroundColor Red
    }
    exit 1
}
