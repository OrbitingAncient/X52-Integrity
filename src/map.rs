//! Best-effort friendly names. HID usages stay authoritative.

use crate::identity::DeviceKind;

/// HID Generic Desktop usage page.
pub const USAGE_PAGE_GENERIC_DESKTOP: u16 = 0x01;
/// HID Button usage page.
pub const USAGE_PAGE_BUTTON: u16 = 0x09;
/// HID Simulation usage page.
pub const USAGE_PAGE_SIMULATION: u16 = 0x02;

/// Generic Desktop usages we care about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum DesktopUsage {
    /// Pointer.
    Pointer = 0x01,
    /// Joystick.
    Joystick = 0x04,
    /// Gamepad.
    Gamepad = 0x05,
    /// X.
    X = 0x30,
    /// Y.
    Y = 0x31,
    /// Z.
    Z = 0x32,
    /// Rx.
    Rx = 0x33,
    /// Ry.
    Ry = 0x34,
    /// Rz (twist).
    Rz = 0x35,
    /// Slider (throttle on many X52 descriptors).
    Slider = 0x36,
    /// Dial.
    Dial = 0x37,
    /// Wheel.
    Wheel = 0x38,
    /// Hat switch.
    Hat = 0x39,
}

/// Label an axis from HID usage.
pub fn axis_name(usage_page: u16, usage: u16, index_among_same: usize) -> String {
    if usage_page == USAGE_PAGE_GENERIC_DESKTOP {
        let base = match usage {
            0x30 => "X",
            0x31 => "Y",
            0x32 => "Z",
            0x33 => "Rx",
            0x34 => "Ry",
            0x35 => "Rz",
            0x36 => "Slider",
            0x37 => "Dial",
            0x38 => "Wheel",
            0x39 => "Hat",
            _ => "",
        };
        if !base.is_empty() {
            return if index_among_same > 0 {
                format!("{base}#{index_among_same}")
            } else {
                base.to_string()
            };
        }
    }
    if usage_page == USAGE_PAGE_SIMULATION && usage == 0xBB {
        return "Throttle".to_string();
    }
    format!("axis:{usage_page:02X}/{usage:02X}")
}

/// True for stick-style centered axes.
pub fn axis_is_centered(usage_page: u16, usage: u16) -> bool {
    usage_page == USAGE_PAGE_GENERIC_DESKTOP && matches!(usage, 0x30 | 0x31 | 0x33 | 0x34 | 0x35)
}

/// Hat direction from a 0..=7 (or 0..=3) value. Idle if `idle` is set.
pub fn hat_direction(raw: u32, logical_min: i32, logical_max: i32, idle: bool) -> String {
    if idle {
        return "idle".to_string();
    }
    let span = logical_max.saturating_sub(logical_min);
    if span <= 0 {
        return format!("raw:{raw}");
    }
    let names_8 = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    let names_4 = ["N", "E", "S", "W"];
    let idx = raw as i32 - logical_min;
    if span >= 7 {
        names_8
            .get(idx as usize)
            .copied()
            .unwrap_or("idle")
            .to_string()
    } else if span >= 3 {
        names_4
            .get(idx as usize)
            .copied()
            .unwrap_or("idle")
            .to_string()
    } else {
        format!("raw:{raw}")
    }
}

/// Best-effort button caption. Falls back to `B##`.
pub fn button_name(kind: DeviceKind, usage: u16) -> &'static str {
    match kind {
        DeviceKind::X52Pro => pro_button_name(usage),
        DeviceKind::X52 => standard_button_name(usage),
        _ => fallback_button_name(usage),
    }
}

fn fallback_button_name(usage: u16) -> &'static str {
    match usage {
        1 => "B01 Trigger",
        2 => "B02 Fire A",
        3 => "B03 Fire B",
        4 => "B04 Fire C",
        5 => "B05 Pinkie",
        27 => "B27 Mode 1",
        28 => "B28 Mode 2",
        29 => "B29 Mode 3",
        n => named_index(n),
    }
}

fn standard_button_name(usage: u16) -> &'static str {
    // Community DirectInput overlay for the standard X52. Display-only.
    match usage {
        1 => "B01 Trigger",
        2 => "B02 Fire A",
        3 => "B03 Fire B",
        4 => "B04 Fire C",
        5 => "B05 Pinkie",
        6 => "B06 T1",
        7 => "B07 T2",
        8 => "B08 T3",
        9 => "B09 T4",
        10 => "B10 T5",
        11 => "B11 T6",
        12 => "B12 D",
        13 => "B13 E",
        14 => "B14 MFD Func",
        15 => "B15 MFD Start/Stop",
        16 => "B16 MFD Reset",
        27 => "B27 Mode 1",
        28 => "B28 Mode 2",
        29 => "B29 Mode 3",
        n => named_index(n),
    }
}

fn pro_button_name(usage: u16) -> &'static str {
    // Community overlay used by DCS / Joystick Gremlin maps for the X52 Pro.
    // Numbers can differ slightly across firmware; raw HID usages always win.
    match usage {
        1 => "B01 Trigger",
        2 => "B02 Fire A",
        3 => "B03 Fire B",
        4 => "B04 Fire C",
        5 => "B05 Pinkie",
        6 => "B06 D",
        7 => "B07 E",
        8 => "B08 T1",
        9 => "B09 T2",
        10 => "B10 T3",
        11 => "B11 T4",
        12 => "B12 T5",
        13 => "B13 T6",
        14 => "B14 MFD Func",
        15 => "B15 MFD Start/Stop",
        16 => "B16 MFD Reset",
        17 => "B17 MFD Pg+",
        18 => "B18 MFD Pg-",
        19 => "B19 MFD Up",
        20 => "B20 MFD Down",
        21 => "B21 Stick hat N",
        22 => "B22 Stick hat E",
        23 => "B23 Stick hat S",
        24 => "B24 Stick hat W",
        25 => "B25 Throttle hat N",
        26 => "B26 Mouse click",
        27 => "B27 Mode 1",
        28 => "B28 Mode 2",
        29 => "B29 Mode 3",
        30 => "B30 Wheel +",
        31 => "B31 Wheel -",
        32 => "B32 Mouse hat N",
        33 => "B33 Mouse hat E",
        34 => "B34 Mouse hat S",
        35 => "B35 Mouse hat W",
        36 => "B36 I",
        37 => "B37 Thumb rot click",
        38 => "B38 Index rot click",
        39 => "B39 Clutch",
        n => named_index(n),
    }
}

fn named_index(n: u16) -> &'static str {
    // Static table so we can return &'static str without leaking.
    const NAMES: &[&str] = &[
        "B00", "B01", "B02", "B03", "B04", "B05", "B06", "B07", "B08", "B09", "B10", "B11", "B12",
        "B13", "B14", "B15", "B16", "B17", "B18", "B19", "B20", "B21", "B22", "B23", "B24", "B25",
        "B26", "B27", "B28", "B29", "B30", "B31", "B32", "B33", "B34", "B35", "B36", "B37", "B38",
        "B39", "B40", "B41", "B42", "B43", "B44", "B45", "B46", "B47", "B48", "B49", "B50", "B51",
        "B52", "B53", "B54", "B55", "B56", "B57", "B58", "B59", "B60", "B61", "B62", "B63",
    ];
    NAMES.get(n as usize).copied().unwrap_or("B??")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hat_idle_and_cardinals() {
        assert_eq!(hat_direction(0, 0, 7, true), "idle");
        assert_eq!(hat_direction(0, 0, 7, false), "N");
        assert_eq!(hat_direction(2, 0, 7, false), "E");
        assert_eq!(hat_direction(4, 0, 7, false), "S");
    }

    #[test]
    fn mode_buttons_named() {
        assert!(button_name(DeviceKind::X52Pro, 27).contains("Mode 1"));
        assert!(button_name(DeviceKind::X52, 1).contains("Trigger"));
    }
}
