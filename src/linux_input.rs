//! Linux live input via evdev (`/dev/input/event*`).
//!
//! Requires the packaged udev rule (or membership in `input`) so a normal
//! user can open the X52 event node. No kernel module is loaded.

#![cfg(unix)]

use evdev::{AbsoluteAxisType, Device, Key};

use crate::error::Error;
use crate::identity::{DeviceKind, SAITEK_VID};
use crate::pipeline::normalize;
use crate::state::{AxisValue, HatValue, ModeSwitch, X52State};

/// One evdev node that matches the X52 allow-list.
#[derive(Debug, Clone)]
pub struct LiveDevice {
    /// `/dev/input/eventN`.
    pub path: String,
    /// USB VID.
    pub vid: u16,
    /// USB PID.
    pub pid: u16,
    /// Device name.
    pub name: String,
    /// Axes advertised.
    pub num_axes: u32,
    /// Buttons advertised.
    pub num_buttons: u32,
    /// Kind.
    pub kind: DeviceKind,
}

/// X52 / X52 Pro devices visible to evdev.
pub fn list_x52() -> Result<Vec<LiveDevice>, Error> {
    let mut out = Vec::new();
    for (path, dev) in evdev::enumerate() {
        let id = dev.input_id();
        let name = dev.name().unwrap_or("X52").to_string();
        let kind = DeviceKind::from_vid_pid(id.vendor, id.product);
        let name_hit = name.to_ascii_lowercase().contains("x52");
        if !kind.is_x52() && id.vendor != SAITEK_VID && !name_hit {
            continue;
        }
        let kind = if kind.is_x52() { kind } else { DeviceKind::X52 };
        let num_axes = dev
            .supported_absolute_axes()
            .map(|s| s.iter().count() as u32)
            .unwrap_or(0);
        let num_buttons = dev
            .supported_keys()
            .map(|s| s.iter().count() as u32)
            .unwrap_or(0);
        out.push(LiveDevice {
            path: path.display().to_string(),
            vid: id.vendor,
            pid: id.product,
            name,
            num_axes,
            num_buttons,
            kind,
        });
        drop(dev);
    }
    Ok(out)
}

/// Read current axis/button state. Uses `get_abs_state` / key state; does not
/// block on the event stream.
pub fn read_state(
    dev: &LiveDevice,
    report_number: u64,
    elapsed_ms: u64,
) -> Result<X52State, Error> {
    let device =
        Device::open(&dev.path).map_err(|e| Error::Platform(format!("evdev open: {e}")))?;
    let abs = device.get_abs_state().unwrap_or_default();
    let keys = device.get_key_state().ok();

    let x = abs_value(&device, &abs, AbsoluteAxisType::ABS_X);
    let y = abs_value(&device, &abs, AbsoluteAxisType::ABS_Y);
    let z = abs_value(&device, &abs, AbsoluteAxisType::ABS_Z);
    let r = abs_value(&device, &abs, AbsoluteAxisType::ABS_RZ)
        .or_else(|| abs_value(&device, &abs, AbsoluteAxisType::ABS_RX));
    let u = abs_value(&device, &abs, AbsoluteAxisType::ABS_RX);
    let v = abs_value(&device, &abs, AbsoluteAxisType::ABS_RY);

    let mut axes = Vec::new();
    push_axis(&mut axes, "X", 0x30, x.unwrap_or(32767), true);
    push_axis(&mut axes, "Y", 0x31, y.unwrap_or(32767), true);
    push_axis(&mut axes, "Z", 0x32, z.unwrap_or(0), false);
    push_axis(&mut axes, "R", 0x35, r.unwrap_or(32767), true);
    push_axis(&mut axes, "U", 0x33, u.unwrap_or(32767), true);
    push_axis(&mut axes, "V", 0x34, v.unwrap_or(32767), true);

    let hatx = abs_value(&device, &abs, AbsoluteAxisType::ABS_HAT0X).unwrap_or(0) as i32;
    let haty = abs_value(&device, &abs, AbsoluteAxisType::ABS_HAT0Y).unwrap_or(0) as i32;
    let (idle, direction) = hat_from_xy(hatx, haty);

    let mut buttons_pressed = Vec::new();
    if let Some(bits) = keys {
        // BTN_TRIGGER is 0x120; we map the first 32 joystick buttons.
        for b in 0..32u16 {
            let code = Key(0x120 + b);
            if bits.contains(code) {
                buttons_pressed.push(b + 1);
            }
        }
    }

    let mut state = X52State {
        report_number,
        elapsed_ms,
        raw: Vec::new(),
        axes,
        hats: vec![HatValue {
            usage_page: 0x01,
            usage: 0x39,
            name: "Hat".into(),
            raw: if idle { 65535 } else { 0 },
            logical_min: 0,
            logical_max: 35900,
            idle,
            direction,
        }],
        buttons_pressed,
        button_names: Vec::new(),
        mode: ModeSwitch::Unknown,
    };
    state = state.finalize(dev.kind);
    Ok(state)
}

fn abs_value(dev: &Device, abs: &[evdev::AbsInfo], code: AbsoluteAxisType) -> Option<u32> {
    let supported = dev.supported_absolute_axes()?;
    if !supported.contains(code) {
        return None;
    }
    // AbsInfo slice is indexed by axis number.
    let idx = code.0 as usize;
    let info = abs.get(idx)?;
    let min = info.min();
    let max = info.max();
    let val = info.value();
    if max <= min {
        return Some(val.clamp(0, 65535) as u32);
    }
    let span = (max - min) as i64;
    let n = ((i64::from(val) - i64::from(min)) * 65535 / span).clamp(0, 65535);
    Some(n as u32)
}

fn hat_from_xy(x: i32, y: i32) -> (bool, String) {
    if x == 0 && y == 0 {
        return (true, "idle".into());
    }
    let dir = match (x.signum(), y.signum()) {
        (0, -1) => "N",
        (1, -1) => "NE",
        (1, 0) => "E",
        (1, 1) => "SE",
        (0, 1) => "S",
        (-1, 1) => "SW",
        (-1, 0) => "W",
        (-1, -1) => "NW",
        _ => "idle",
    };
    (dir == "idle", dir.into())
}

fn push_axis(out: &mut Vec<AxisValue>, name: &str, usage: u16, raw: u32, centered: bool) {
    let normalized = normalize(raw, 0, 65_535, centered);
    out.push(AxisValue {
        usage_page: 0x01,
        usage,
        name: name.to_string(),
        raw,
        logical_min: 0,
        logical_max: 65_535,
        normalized,
        processed: normalized,
        centered,
    });
}

/// Drain pending events so the kernel queue does not fill.
pub fn pump(dev: &LiveDevice) -> Result<(), Error> {
    let mut device =
        Device::open(&dev.path).map_err(|e| Error::Platform(format!("evdev open: {e}")))?;
    if let Ok(iter) = device.fetch_events() {
        for _ in iter.flatten() {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hat_idle_center() {
        let (idle, dir) = hat_from_xy(0, 0);
        assert!(idle);
        assert_eq!(dir, "idle");
    }

    #[test]
    fn hat_north() {
        let (idle, dir) = hat_from_xy(0, -1);
        assert!(!idle);
        assert_eq!(dir, "N");
    }
}
