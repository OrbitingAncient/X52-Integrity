# X52 Integrity

User-space companion for the Saitek/Logitech **X52** and **X52 Pro**. It runs on Windows 10/11 with Memory Integrity and HVCI left on, and on Linux via evdev.

This is software, not a firmware replacement. The HOTAS keeps its factory firmware. The process runs as a normal user and never auto-elevates. Optional Windows driver buttons show a confirm dialog, then UAC.

[Install on Windows](docs/install-windows.md) · [Install on Linux](docs/install-linux.md) · [Architecture](docs/architecture.md) · [Release](docs/RELEASE.md)

## Features

- Live stick, throttle, twist, rotaries, hat, and buttons
- Windowed manager: status, activity log, install / uninstall / test
- Console monitor (`x52-monitor`) for diagnose, JSON snapshots, and a dashboard
- Per-user install (no admin) plus portable zip and silent CLI
- Windows: bind the device to inbox HID, or restore an exported Logitech package, only after you confirm
- Linux: udev rule + `.deb` / AppImage
- Deny-by-default safety gate; vendor LED/MFD and virtual-joystick output are compiled out

VID `0x06A3`, PIDs `0x0255` / `0x075C` (X52) and `0x0762` (X52 Pro). Plug the **throttle** USB cable; the stick uses the internal cable.

## Architecture

```
Physical X52 USB
       │
       ▼
Windows: hidusb.sys / hidclass.sys     (HVCI / Memory Integrity OK)
Linux:   hid-generic + evdev + udev
       │
       ▼
x52-integrity  (normal user process)
  Enumerator → live input (winmm / evdev) → curves → UI
  SafetyGate deny-by-default
```

Windows 11 `hidgamepad.inf` owns interrupt HID reports, so live axes come from inbox `winmm` (`joyGetPosEx`). That is expected and HVCI-safe. Linux reads `/dev/input/event*`.

vJoy is not used. A virtual device, if added later, would be **ViGEmBus only** (WHQL) after an explicit install you confirm.

## Install — Windows

Supported: current Windows 10, and Windows 11 24H2 through 26H2, including Memory Integrity, HVCI, and Smart App Control.

**Portable:** unzip `X52-Integrity-*-windows-x64-portable.zip` and run `X52 Integrity.cmd`.

**Per-user (no admin):**

```powershell
# From a GitHub release, after Unblock-File
.\x52-integrity.exe --install --silent

# or
powershell.exe -NoProfile -ExecutionPolicy Bypass -File Install-X52Integrity.ps1
```

**Per-machine:** `Install-X52Integrity.ps1 -Machine` (UAC) → `%ProgramFiles%\X52 Integrity`.

Default data directory: `%LOCALAPPDATA%\X52Integrity`. Override with `X52_INTEGRITY_HOME` or `--prefix`.

If the first install looked like it failed, the app was probably already copied and only the shortcut step broke. That is fixed: shortcut errors are warnings, PowerShell is invoked by full path, and `logs\install.log` records the transaction. Details: [docs/install-windows.md](docs/install-windows.md).

## Install — Linux

```bash
sudo apt install ./x52-integrity_*.deb    # also installs udev rules
# or
chmod +x X52-Integrity-*-x86_64.AppImage
sudo cp packaging/linux/99-x52-integrity.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```

Data: `~/.local/share/x52-integrity`. See [docs/install-linux.md](docs/install-linux.md).

## Build from source

Rust 1.80+ (`rustup`).

```powershell
cargo test
cargo build --release
.\target\release\x52-integrity.exe
.\target\release\x52-monitor.exe --diagnose
```

```bash
cargo test
cargo build --release
./target/release/x52-integrity
./target/release/x52-monitor --diagnose
```

Linux GUI needs a working X11 or Wayland session and the usual winit/gtk build packages (`libgtk-3-dev`, `libxkbcommon-dev`, `libwayland-dev` on Debian).

## Troubleshooting

| Symptom | What to do |
|---|---|
| Install said it failed, but the exe is in `%LOCALAPPDATA%\X52Integrity` | Shortcuts only. Run the exe from that folder or `--install` again. Read `logs\install.log`. |
| `powershell` / “program not found” | Fixed in 0.2. Use this tree, not an old `x52-firmfix` build. |
| SmartScreen / Smart App Control block | Unblock-File; prefer a signed NSIS setup (`docs/RELEASE.md`). Do not turn off Memory Integrity. |
| No live axes on Windows | Use inbox HID (GUI **Windows HID**). Logitech `SaiK075C` filters steal interrupt traffic. |
| Access denied on Linux `/dev/input/event*` | Install the udev rule, replug, or add your user to `input` and re-login. |
| Device not listed | Throttle USB only. VID/PID must be 06A3:0255, 075C, or 0762. |

`x52-integrity --self-test` prints driver stack, privilege, and a live snapshot.

## Safety

- Default mode is monitor-only. `--led`, `--flash`, `--vjoy`, `--arm` are rejected.
- The GUI never requests UAC on launch.
- Cargo feature `write_path` is off. Do not enable it unless we discuss it.
- What the code will never do: [docs/what-this-will-never-do.md](docs/what-this-will-never-do.md).

## License

[MIT](LICENSE). Logitech/Saitek driver binaries are **not** shipped. Export your own package before switching to inbox HID if you want a restore path (`tools/restore-saitek-x52-driver.ps1`).
