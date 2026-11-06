# Dependencies

None of these modify the machine permanently. None of them install a driver.

## Required to build

| Dependency | Kind | System-wide install? |
|---|---|---|
| Rust 1.80+ (`rustc`, `cargo`) | Toolchain already on this machine | No additional install by this project |
| Windows 11 inbox `hid.dll`, `setupapi.dll`, `kernel32.dll`, `advapi32.dll` | OS libraries | Already present; not replaced |

The .NET SDK is **not** required. Rust was chosen because a .NET SDK is not installed here and the user brief allows an equivalent Rust toolchain.

## Cargo crates (user-space, crates.io)

| Crate | Why | Native installer? |
|---|---|---|
| `windows-sys` 0.59 | Typed FFI to inbox Win32 HID / SetupAPI / console / registry **reads**; confirmed driver helper uses `newdev` + `ShellExecuteEx` | No |
| `serde` 1 | Profile JSON | No |
| `serde_json` 1 | Profile JSON | No |
| `eframe` 0.31 (glow) | Windowed status / logs / setup UI (user-space OpenGL) | No |
| `thiserror` 2 | Typed errors on the library path | No |
| `evdev` 0.12 (Linux) | Read `/dev/input/event*` | No |
| `libc` 0.2 (Linux) | `geteuid`, signals, localtime | No |

`cargo build` downloads crate sources into `target/` and the local cargo registry. That is per-user, not a system driver install.

## Explicitly not depended on

- libusb, WinUSB rebind helpers, libusbK
- vJoy (not used; not HVCI-safe)
- ViGEmClient / ViGEmBus (optional later, WHQL only, not installed by this repo)
- Logitech DirectOutput / SST
- Any INF, `.sys`, `.cat`, or custom HID filter

## Optional future write-path (not in this build)

Virtual joystick output would require a **separate**, user-confirmed install of a signed bus driver such as ViGEmBus (WHQL, generally Memory Integrity compatible). That is not a dependency of `x52-monitor` and is not installed by this repo.
