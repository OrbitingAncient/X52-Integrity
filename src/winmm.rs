//! Windows multimedia joystick API (`winmm.dll`).
//!
//! User-space, inbox, HVCI-safe. On Windows 11 the hidgamepad extension often
//! delivers reports here (and to DirectInput) while a second HID `ReadFile`
//! handle sees no interrupt traffic. We use this as the live-state source
//! when HID interrupt reads time out.

use std::mem::size_of;

use crate::error::Error;
use crate::identity::{DeviceKind, SAITEK_VID};
use crate::pipeline::normalize;
use crate::state::{AxisValue, HatValue, ModeSwitch, X52State};

const JOYERR_NOERROR: u32 = 0;
const JOY_RETURNALL: u32 = 0x00FF;
const JOY_POVCENTERED: u32 = 65_535;
const MAX_JOYSTICKS: u32 = 16;

#[repr(C)]
#[derive(Clone, Copy)]
struct JoyInfoEx {
    size: u32,
    flags: u32,
    x: u32,
    y: u32,
    z: u32,
    r: u32,
    u: u32,
    v: u32,
    buttons: u32,
    button_number: u32,
    pov: u32,
    reserved1: u32,
    reserved2: u32,
}

#[repr(C)]
struct JoyCapsW {
    mid: u16,
    pid: u16,
    pname: [u16; 32],
    x_min: u32,
    x_max: u32,
    y_min: u32,
    y_max: u32,
    z_min: u32,
    z_max: u32,
    num_buttons: u32,
    period_min: u32,
    period_max: u32,
    r_min: u32,
    r_max: u32,
    u_min: u32,
    u_max: u32,
    v_min: u32,
    v_max: u32,
    caps: u32,
    max_axes: u32,
    num_axes: u32,
    max_buttons: u32,
    reg_key: [u16; 32],
    oem_vx_d: [u16; 260],
}

#[link(name = "winmm")]
extern "system" {
    fn joyGetNumDevs() -> u32;
    fn joyGetPosEx(id: u32, info: *mut JoyInfoEx) -> u32;
    fn joyGetDevCapsW(id: u32, caps: *mut JoyCapsW, size: u32) -> u32;
}

/// One multimedia joystick that matches the X52 allow-list.
#[derive(Debug, Clone)]
pub struct WinmmDevice {
    /// `joyGetPosEx` identifier (0..=15).
    pub id: u32,
    /// USB VID from `JOYCAPS`.
    pub vid: u16,
    /// USB PID from `JOYCAPS`.
    pub pid: u16,
    /// Product name.
    pub name: String,
    /// Axes advertised.
    pub num_axes: u32,
    /// Buttons advertised.
    pub num_buttons: u32,
    /// Kind.
    pub kind: DeviceKind,
}

fn wide_name(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// X52 / X52 Pro devices visible to winmm.
pub fn list_x52() -> Result<Vec<WinmmDevice>, Error> {
    let n = unsafe { joyGetNumDevs() }.min(MAX_JOYSTICKS);
    let mut out = Vec::new();
    for id in 0..n {
        let mut caps = unsafe { std::mem::zeroed::<JoyCapsW>() };
        let st = unsafe { joyGetDevCapsW(id, &mut caps, size_of::<JoyCapsW>() as u32) };
        if st != JOYERR_NOERROR {
            continue;
        }
        let name = wide_name(&caps.pname);
        let kind = DeviceKind::from_vid_pid(caps.mid, caps.pid);
        let name_hit = name.to_ascii_lowercase().contains("x52");
        if !kind.is_x52() && caps.mid != SAITEK_VID && !name_hit {
            continue;
        }
        let kind = if kind.is_x52() { kind } else { DeviceKind::X52 };
        out.push(WinmmDevice {
            id,
            vid: caps.mid,
            pid: caps.pid,
            name: wide_name(&caps.pname),
            num_axes: caps.num_axes,
            num_buttons: caps.num_buttons,
            kind,
        });
    }
    Ok(out)
}

/// Read one live snapshot. Axes are 0..=65535 in this API (center 32767).
pub fn read_state(
    dev: &WinmmDevice,
    report_number: u64,
    elapsed_ms: u64,
) -> Result<X52State, Error> {
    let mut info = JoyInfoEx {
        size: size_of::<JoyInfoEx>() as u32,
        flags: JOY_RETURNALL,
        x: 0,
        y: 0,
        z: 0,
        r: 0,
        u: 0,
        v: 0,
        buttons: 0,
        button_number: 0,
        pov: JOY_POVCENTERED,
        reserved1: 0,
        reserved2: 0,
    };
    let st = unsafe { joyGetPosEx(dev.id, &mut info) };
    if st != JOYERR_NOERROR {
        return Err(Error::Win32 {
            api: "joyGetPosEx",
            code: st,
        });
    }

    let mut axes = Vec::new();
    push_axis(&mut axes, "X", 0x30, info.x, true);
    push_axis(&mut axes, "Y", 0x31, info.y, true);
    push_axis(&mut axes, "Z", 0x32, info.z, false);
    push_axis(&mut axes, "R", 0x35, info.r, true);
    push_axis(&mut axes, "U", 0x33, info.u, true);
    push_axis(&mut axes, "V", 0x34, info.v, true);

    let idle = info.pov == JOY_POVCENTERED;
    let direction = if idle {
        "idle".to_string()
    } else {
        // dwPOV is hundredths of a degree, 0 = N, 9000 = E.
        let deg = (info.pov / 100) % 360;
        let idx = ((deg + 22) / 45) % 8;
        ["N", "NE", "E", "SE", "S", "SW", "W", "NW"][idx as usize].to_string()
    };
    let hats = vec![HatValue {
        usage_page: 0x01,
        usage: 0x39,
        name: "Hat".to_string(),
        raw: info.pov,
        logical_min: 0,
        logical_max: 35900,
        idle,
        direction,
    }];

    let mut buttons_pressed = Vec::new();
    for b in 0..dev.num_buttons.min(32) {
        if info.buttons & (1 << b) != 0 {
            buttons_pressed.push((b + 1) as u16);
        }
    }

    let mut state = X52State {
        report_number,
        elapsed_ms,
        raw: vec![
            (info.x & 0xFF) as u8,
            ((info.x >> 8) & 0xFF) as u8,
            (info.y & 0xFF) as u8,
            ((info.y >> 8) & 0xFF) as u8,
            (info.z & 0xFF) as u8,
            ((info.z >> 8) & 0xFF) as u8,
            (info.r & 0xFF) as u8,
            ((info.r >> 8) & 0xFF) as u8,
            (info.u & 0xFF) as u8,
            ((info.u >> 8) & 0xFF) as u8,
            (info.v & 0xFF) as u8,
            ((info.v >> 8) & 0xFF) as u8,
            (info.buttons & 0xFF) as u8,
            ((info.buttons >> 8) & 0xFF) as u8,
            (info.pov & 0xFF) as u8,
            ((info.pov >> 8) & 0xFF) as u8,
        ],
        axes,
        hats,
        buttons_pressed,
        button_names: Vec::new(),
        mode: ModeSwitch::Unknown,
    };
    state = state.finalize(dev.kind);
    Ok(state)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pov_idle_constant() {
        assert_eq!(JOY_POVCENTERED, 65_535);
    }
}
