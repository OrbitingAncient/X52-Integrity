# Restore Logitech sai075c.inf (oem237) on the X52. Inverse of use-inbox-hid.ps1.

param(
    [switch]$WhatIf
)

$ErrorActionPreference = 'Stop'
$usbHwId = 'USB\VID_06A3&PID_075C'
$candidates = @(
    'H:\Projects\X52 Firmfix\tools\backups\sai075c\sai075c.inf',
    'C:\Windows\System32\DriverStore\FileRepository\sai075c.inf_amd64_361a3d1b47a342af\sai075c.inf'
)
$inf = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $inf) {
    throw "sai075c.inf backup not found. Export it with: pnputil /export-driver oem237.inf tools\backups\sai075c"
}

$INSTALLFLAG_FORCE = 0x1
$INSTALLFLAG_NONINTERACTIVE = 0x4
$flags = $INSTALLFLAG_FORCE -bor $INSTALLFLAG_NONINTERACTIVE

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class X52NewdevRestore {
    [DllImport("newdev.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern bool UpdateDriverForPlugAndPlayDevices(
        IntPtr hwndParent,
        string HardwareId,
        string FullInfPath,
        uint InstallFlags,
        out bool bRebootRequired);
}
"@

Write-Host "Restoring $usbHwId -> $inf"
if ($WhatIf) { exit 0 }
$reboot = $false
$ok = [X52NewdevRestore]::UpdateDriverForPlugAndPlayDevices(
    [IntPtr]::Zero, $usbHwId, $inf, [uint32]$flags, [ref]$reboot)
$err = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
if (-not $ok) {
    throw "Restore failed win32=$err"
}
Write-Host "  ok rebootRequired=$reboot"
pnputil /scan-devices | Out-Host
