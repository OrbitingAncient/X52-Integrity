# Windows install and troubleshooting

X52 Integrity is a **user-space** app. It does not install a kernel driver. It works with Memory Integrity, HVCI, and Smart App Control left on.

Supported: up-to-date Windows 10 (22H2 / 21H2 with current cumulative updates) and Windows 11 24H2, 25H2, and 26H2 (x64).

## Choose a distribution

| Package | Admin? | Shortcuts | Typical use |
|---|---|---|---|
| In-app **Install** button, or `x52-integrity.exe --install` | No | Desktop + Start Menu | Daily per-user install |
| `Install-X52Integrity.ps1` | No (yes with `-Machine`) | Same | Scripted / IT |
| `X52-Integrity-*-setup.exe` (NSIS) | No | Same | Signed installer |
| `*-windows-x64-portable.zip` | No | No | USB stick / no-write profile |

Default location: `%LOCALAPPDATA%\X52Integrity`.

Per-machine: `%ProgramFiles%\X52 Integrity` with `-Machine` or `X52_INTEGRITY_HOME`.

## Interactive

1. Unblock the download if Windows marked it: right-click → Properties → Unblock, or `Unblock-File`.
2. Run `x52-integrity.exe`.
3. First-run panel: connect the **throttle** USB cable, confirm live axes, Continue.
4. **Install** copies the app and creates shortcuts. Shortcut problems are warnings; the files still work.

```powershell
# From a built tree
.\target\release\x52-integrity.exe --install
```

## Silent

```powershell
.\x52-integrity.exe --install --silent
.\x52-integrity.exe --install --silent --prefix "D:\Apps\X52Integrity"
.\x52-integrity.exe --uninstall --silent
```

PowerShell wrapper (creates shortcuts in-process, writes `logs\install.log`):

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File packaging\windows\Install-X52Integrity.ps1 -Silent
```

Exit codes: `0` success (including shortcut warnings), `1` copy failure (rolled back), `2` usage.

Logs:

- `%LOCALAPPDATA%\X52Integrity\logs\x52-integrity.log` — app
- `%LOCALAPPDATA%\X52Integrity\logs\install.log` — installer
- `%LOCALAPPDATA%\X52Integrity\install-manifest.json` — what was written (used by uninstall / rollback)

## Why the first install “failed”

The original GUI installer spawned `powershell` to write `.lnk` files. Nested processes often have **no PATH**, so `powershell` was “program not found” and the UI treated that as a hard failure even though the exe had already been copied.

Current behavior:

- Rust installer calls `C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe`, then `cscript.exe`.
- Shortcut failure is a **warning**. Files stay. Manifest is written.
- File-copy failure **rolls back**.
- The PowerShell / NSIS installers create shortcuts themselves (COM / `CreateShortCut`) and never spawn a nested `powershell`.

If shortcuts are still missing, the app at `%LOCALAPPDATA%\X52Integrity\x52-integrity.exe` is valid. Pin it or rerun `--install`.

## Smart App Control / SmartScreen / antivirus

Unsigned downloads are blocked or warned on a clean Windows 11 box.

1. Prefer the **NSIS** setup EXE once it is Authenticode-signed (`docs/RELEASE.md`).
2. `Unblock-File` on the zip and the exe (clears Mark of the Web).
3. Smart App Control in **Evaluation** allows new apps after a while; in **Enforcement** it needs a signature or a Microsoft-signed catalog. There is no supported way around that except signing.
4. If Defender quarantines the exe, restore it and submit a false-positive. The binary is userspace Rust; it does not load a `.sys`.
5. Do not disable Memory Integrity or HVCI.

## Memory Integrity / HVCI

Leave them on. This app talks to inbox `hidusb.sys` / `hidclass.sys` and `winmm` `joyGetPosEx`. Optional driver buttons bind the X52 to inbox `input.inf` or restore a **previously exported** Logitech package. They never ship an unsigned kernel driver.

## Existing X52 software

Install does **not** uninstall Logitech Gaming Software, SST, or LSaiBus. Profiles and bindings those tools stored are left alone. Binding to Windows HID is a separate, confirmed, UAC-gated action.

## Portable zip

```
X52 Integrity.cmd     sets X52_INTEGRITY_HOME to this folder
x52-integrity.exe
x52-monitor.exe
```

Logs and `config.json` live in that folder.

## Command line

```
x52-integrity.exe                 GUI
x52-integrity.exe --self-test     device + input snapshot, then exit
x52-integrity.exe --install [--silent] [--prefix DIR]
x52-integrity.exe --uninstall [--silent] [--prefix DIR]
x52-integrity.exe --driver-inbox      administrator helper
x52-integrity.exe --driver-restore    administrator helper
```
