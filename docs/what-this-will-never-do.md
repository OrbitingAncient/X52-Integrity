# What this code will never do

This repository is a **user-space software** companion for the Saitek/Logitech X52 and X52 Pro. It is not firmware, and it will not become firmware.

The process will never:

1. Flash, update, replace, or patch the X52’s onboard firmware.
2. Issue undocumented bootloader commands.
3. Install, sign, or load a **custom / unsigned** kernel driver. Confirmed GUI buttons may bind the X52 to Windows inbox `input.inf` or restore the exported Logitech `sai075c.inf` backup.
4. Ship a new INF, catalog, or driver package. The GUI may apply inbox `input.inf` or the exported `sai075c.inf` backup after you confirm and accept UAC.
5. Rebind the joystick USB interface from `hidusb.sys` to WinUSB, libusbK, or any other function driver.
6. Modify Windows system files.
7. Modify protected registry keys (DeviceGuard, driver policy, services).
8. Auto-elevate. The GUI never requests UAC on launch. Driver buttons show a confirm dialog, then `ShellExecute` `runas` so Windows can show UAC.
9. Disable or ask you to disable Memory Integrity, HVCI, Kernel-mode Hardware-enforced Stack Protection, or test-signing requirements.
10. Send vendor control transfers (request `0x91`) in monitor-only mode.
11. Install or talk to vJoy.
12. Write to a virtual joystick in monitor-only mode. ViGEmBus is the only virtual-device option if you later confirm an install.
13. Inject keyboard, mouse, or macro events in monitor-only mode.

`x52-monitor` has no code path for items 1–6 and 10–13. Items 7–9 are also excluded from diagnose, which is read-only. `x52-integrity` can rebind the X52 to inbox HID or restore the exported Logitech package only after a confirm dialog and UAC. Switching the X52 to inbox `HidUsb` is an explicit, reversible driver-store change (local backup in `tools/backups`, not published).
