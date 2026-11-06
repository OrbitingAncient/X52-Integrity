# Contributing

## Development

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings -W clippy::pedantic
cargo test
```

MSRV is 1.80. Edition 2021.

## Rules

- User-space only. No new kernel drivers, INF files, or unsigned `.sys`.
- Do not flash X52 firmware or send undocumented bootloader commands.
- Hardware writes (LED/MFD, virtual joystick) stay behind `write_path` **and** `SafetyGate`.
- vJoy is out of scope. ViGEmBus is the only virtual-device option, and it is not installed by this repo.
- Library code returns `Error` (`thiserror`). No `unwrap` / `expect` on the release path. Tests may use them.
- Comment non-obvious constraints (HVCI, HID interrupt silence on Windows 11, udev `uaccess`).
- Installer changes must keep shortcut failure as a warning, log to `logs/install.log`, and roll back on copy failure.

## Pull requests

1. One concern per PR when you can.
2. Update `docs/` if you change install, safety, or the driver bind path.
3. CI must pass on `windows-latest` and `ubuntu-latest`.

## Code layout

| Path | Role |
|---|---|
| `src/lib.rs` | Crate root, `VERSION`, module list |
| `src/hid/` | Windows SetupAPI/HID, Linux evdev |
| `src/live.rs` | OS joystick snapshot (winmm / evdev) |
| `src/install.rs` | Transactional per-user copy, shortcuts, rollback |
| `src/setup.rs` | Windows driver bind (UAC after confirm) |
| `src/bin/x52-integrity/` | GUI + `--install` / `--silent` |
| `src/main.rs` | `x52-monitor` |
| `packaging/` | NSIS, PowerShell, udev, AppImage, desktop file |
