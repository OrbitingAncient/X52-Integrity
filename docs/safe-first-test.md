# Safe first test (monitor-only)

This checklist never sends a vendor command, never flashes firmware, and never installs a driver. Leave Logitech/Saitek software installed.

## 0. Preconditions

- Windows 11, normal user account (no “Run as administrator”).
- Memory Integrity / HVCI may stay **on**. Do not turn it off for this test.
- X52 or X52 Pro plugged in (one USB cable; stick is chained internally).
- Existing X52 drivers/software may remain. We will not uninstall them in this test.

## 0b. Windowed tool

```powershell
.\target\release\x52-firmfix.exe
```

Expected: a window with status, live axes, a log pane, and **Test** / **Install app** / **Remove app** / driver buttons. Close it any time. **Test** is read-only. **Install app** is per-user (`%LOCALAPPDATA%\X52Firmfix`) and does not request UAC. Driver buttons show a confirm dialog first.

```powershell
.\target\release\x52-firmfix.exe --self-test
```

Expected: diagnose + winmm snapshot printed, no driver changes.

## 1. Build

From the repo root:

```powershell
cargo test
cargo build --release
```

Expected: all unit tests pass without hardware. Binaries: `target\release\x52-firmfix.exe` (window) and `target\release\x52-monitor.exe` (console).

## 2. Help and refused flags

```powershell
.\target\release\x52-monitor.exe --help
.\target\release\x52-monitor.exe --led
.\target\release\x52-monitor.exe --flash
```

Expected: help text. `--led` / `--flash` print a refusal and exit nonzero.

## 3. Read-only diagnose

```powershell
.\target\release\x52-monitor.exe --diagnose
```

Expected:

- Privilege is Standard user on a normal prompt, Administrator if the parent was already elevated
- Memory Integrity On / Off / Unknown
- X52 listed with Windows HID (or Logitech filters if that stack is bound)

## 4. List the device

```powershell
.\target\release\x52-monitor.exe --list
```

Expected: one line with `VID=06A3` and PID `0255`, `075C`, or `0762`.

If none: use `--list-all-hid` to see whether Windows enumerates the device at all. Still read-only.

## 5. One-shot decode

```powershell
.\target\release\x52-monitor.exe --once
.\target\release\x52-monitor.exe --once --json
```

Expected: one parsed report (axes, hats, buttons, raw hex). Move the stick while running `--once` if the first capture is idle.

## 6. Live dashboard

```powershell
.\target\release\x52-monitor.exe
```

Move stick, throttle, hats, mode rotary, buttons. Values should change. Ctrl+C exits. Stderr should say `ExitGuard: no hardware writes were issued; nothing to restore.`

Optional profile (write flags ignored):

```powershell
.\target\release\x52-monitor.exe --profile .\profiles\example-monitor-only.json
```

## 7. What success looks like

- Device is visible as a standard HID joystick.
- Reports decode without any vendor transfer.
- LEDs and MFD are untouched (whatever state Logitech left them in).
- Memory Integrity was not asked to change.

## 8. Stop

Unplug the device or quit the process. Nothing to uninstall. Nothing to restore.

Do **not** proceed to LED/MFD, vJoy/ViGEm, or Logitech uninstall until we confirm those steps together.
