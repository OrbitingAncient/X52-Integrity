# Steps that need your confirmation

I will not run these unless you reply and approve the specific step.

| Step | Why it is gated | Needed for monitor-only? |
|---|---|---|
| Restore Logitech `sai075c.inf` | `tools/restore-saitek-x52-driver.ps1` puts SaiK075C back | Already switched to inbox HID |
| Disable LSaiBus / LSaiMini (Saitek keyboard/mouse programming stack) | Remaining 2017 kernel drivers, separate from the X52 joystick | No |
| Install ViGEmBus (WHQL, HVCI-safe) | Only virtual-joystick option this project will ever use | No |
| Install vJoy | Not HVCI-safe; will not be used | Never |
| `cargo build --features write_path` | Compiles send symbols that are still gated at runtime | No |
| Send vendor `0x91` (LED/MFD brightness or text) | Hardware write; may not work on a HID-bound interface without a driver we refuse to install | No |
| Install ViGEmBus | System-wide signed kernel bus; generally HVCI-OK | No |
| Install vJoy | System-wide kernel bus; older builds break Memory Integrity | No |
| Rebind the X52 from `hidusb` to WinUSB | Breaks standard joystick HID; requires an INF | No, and we will not do this |
| Flash or replace device firmware | Forbidden | Never |

Safe to run without further confirmation: `cargo test`, `cargo build --release`, `x52-firmfix` window, **Test**, **Install app** / **Remove app** (per-user), `x52-monitor --diagnose`, `--list`, `--once`, live dashboard.

Driver buttons in the GUI (**Use inbox HID**, **Restore Logitech**) are confirmed in the window and then UAC. They are the same gated steps as the scripts in this table.
