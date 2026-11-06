//! USB identity of the X52 family. VID/PID allow-list.

/// Saitek (Logitech) USB vendor ID.
pub const SAITEK_VID: u16 = 0x06A3;

/// Older X52 Flight Control System.
pub const PID_X52_LEGACY: u16 = 0x0255;

/// X52 (standard).
pub const PID_X52: u16 = 0x075C;

/// X52 Pro.
pub const PID_X52_PRO: u16 = 0x0762;

/// PIDs this crate treats as an X52 HOTAS. Other Saitek devices are ignored.
pub const X52_PIDS: [u16; 3] = [PID_X52_LEGACY, PID_X52, PID_X52_PRO];

/// Classification of a HID device we looked at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    /// PID 0x0255 or 0x075C.
    X52,
    /// PID 0x0762.
    X52Pro,
    /// VID matches Saitek but PID is not in the X52 allow-list.
    OtherSaitek,
    /// Not a Saitek device.
    NotSaitek,
}

impl DeviceKind {
    /// Classify a VID/PID pair.
    pub fn from_vid_pid(vid: u16, pid: u16) -> Self {
        if vid != SAITEK_VID {
            return DeviceKind::NotSaitek;
        }
        match pid {
            PID_X52_LEGACY | PID_X52 => DeviceKind::X52,
            PID_X52_PRO => DeviceKind::X52Pro,
            _ => DeviceKind::OtherSaitek,
        }
    }

    /// True for standard X52 and X52 Pro.
    pub fn is_x52(self) -> bool {
        matches!(self, DeviceKind::X52 | DeviceKind::X52Pro)
    }

    /// Short display name.
    pub fn display_name(self) -> &'static str {
        match self {
            DeviceKind::X52 => "X52",
            DeviceKind::X52Pro => "X52 Pro",
            DeviceKind::OtherSaitek => "Other Saitek HID",
            DeviceKind::NotSaitek => "HID",
        }
    }
}

/// Format a USB ID as `VID=06A3 PID=0762`.
pub fn format_vid_pid(vid: u16, pid: u16) -> String {
    format!("VID={vid:04X} PID={pid:04X}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_known_pids() {
        assert_eq!(
            DeviceKind::from_vid_pid(SAITEK_VID, PID_X52_PRO),
            DeviceKind::X52Pro
        );
        assert_eq!(
            DeviceKind::from_vid_pid(SAITEK_VID, PID_X52),
            DeviceKind::X52
        );
        assert_eq!(
            DeviceKind::from_vid_pid(SAITEK_VID, PID_X52_LEGACY),
            DeviceKind::X52
        );
        assert_eq!(
            DeviceKind::from_vid_pid(SAITEK_VID, 0x0C2D),
            DeviceKind::OtherSaitek
        );
        assert_eq!(
            DeviceKind::from_vid_pid(0x046D, PID_X52_PRO),
            DeviceKind::NotSaitek
        );
    }

    #[test]
    fn allow_list_is_exactly_the_three_documented_pids() {
        assert!(X52_PIDS.contains(&0x0255));
        assert!(X52_PIDS.contains(&0x075C));
        assert!(X52_PIDS.contains(&0x0762));
        assert_eq!(X52_PIDS.len(), 3);
    }
}
