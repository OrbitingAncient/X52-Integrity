# Linux install

X52 Integrity reads the X52 through **evdev** (`/dev/input/event*`). No kernel module is loaded. The packaged udev rule tags the device `uaccess` so a logged-in user can open it.

VID `06a3`, PIDs `0255` / `075c` (X52) and `0762` (X52 Pro). Plug the **throttle** USB cable.

## Debian / Ubuntu (.deb)

```bash
sudo apt install ./x52-integrity_0.2.0_amd64.deb
# or, from a source tree with cargo-deb:
cargo install cargo-deb
cargo deb
sudo apt install ./target/debian/x52-integrity_*.deb
```

The package installs:

- `/usr/bin/x52-integrity`, `/usr/bin/x52-monitor`
- `/usr/share/applications/x52-integrity.desktop`
- `/usr/lib/udev/rules.d/99-x52-integrity.rules`

Then unplug and replug the X52 (or `sudo udevadm trigger`).

## AppImage

```bash
chmod +x X52-Integrity-0.2.0-x86_64.AppImage
./X52-Integrity-0.2.0-x86_64.AppImage
```

Install the udev rule once so the AppImage can open the device without root:

```bash
sudo cp packaging/linux/99-x52-integrity.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
```

Data: `~/.local/share/x52-integrity` (override with `X52_INTEGRITY_HOME`).

## From source

```bash
sudo apt install build-essential pkg-config libgtk-3-dev libxkbcommon-dev libwayland-dev
# or the equivalent on Fedora: gtk3-devel libxkbcommon-devel wayland-devel
cargo build --release
sudo cp packaging/linux/99-x52-integrity.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
./target/release/x52-integrity
./target/release/x52-monitor --diagnose
```

If the GUI cannot open a display, the compositor needs X11 or Wayland (eframe/winit). The monitor binary is console-only.

## Permission denied on `/dev/input/event*`

1. Confirm the rule loaded: `udevadm info -a -n /dev/input/eventN | grep 06a3`
2. Your user should be in group `input` **or** the device should have `uaccess` (systemd-logind seat).
3. Log out and back in after joining `input`.
4. Do not run the GUI as root.

## First run

The window offers a one-time welcome card. Connect the throttle, wait for live axes, Continue. That writes `config.json` with a coarse idle/center snapshot.
