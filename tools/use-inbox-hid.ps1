# Switch the X52 from Logitech oem237 (SaiK075C/SaiU075C) to Windows inbox input.inf.
# Reversible. Does not uninstall the Logitech package from the driver store.
# Requires an already-elevated session. This script never enables Memory Integrity itself.

param(
    [switch]$WhatIf
)

$ErrorActionPreference = 'Stop'
$usbHwId = 'USB\VID_06A3&PID_075C'
$hidHwId = 'HID\VID_06A3&PID_075C'
$inf = 'C:\Windows\INF\input.inf'
$INSTALLFLAG_FORCE = 0x1
$INSTALLFLAG_NONINTERACTIVE = 0x4
$flags = $INSTALLFLAG_FORCE -bor $INSTALLFLAG_NONINTERACTIVE

if (-not (Test-Path $inf)) {
    throw "Inbox HID INF missing: $inf"
}

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class X52Newdev {
    [DllImport("newdev.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern bool UpdateDriverForPlugAndPlayDevices(
        IntPtr hwndParent,
        string HardwareId,
        string FullInfPath,
        uint InstallFlags,
        out bool bRebootRequired);
}
"@

function Update-HwId([string]$hwid) {
    Write-Host "Updating $hwid -> $inf (FORCE | NONINTERACTIVE)"
    if ($WhatIf) { return }
    $reboot = $false
    $ok = [X52Newdev]::UpdateDriverForPlugAndPlayDevices(
        [IntPtr]::Zero, $hwid, $inf, [uint32]$flags, [ref]$reboot)
    $err = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
    if (-not $ok) {
        throw "UpdateDriverForPlugAndPlayDevices($hwid) failed win32=$err"
    }
    Write-Host "  ok rebootRequired=$reboot"
}

Update-HwId $usbHwId
Update-HwId $hidHwId

if (-not $WhatIf) {
    $usb = Get-PnpDevice | Where-Object { $_.InstanceId -like 'USB\VID_06A3&PID_075C*' }
    foreach ($d in $usb) {
        Write-Host "Restarting $($d.InstanceId)"
        pnputil /restart-device $d.InstanceId | Out-Host
    }
}

Write-Host "Done. Verify with: pnputil /enum-devices /instanceid USB\VID_06A3&PID_075C\9&12FC30F1&0&1 /drivers /stack"
