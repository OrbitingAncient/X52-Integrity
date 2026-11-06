//! OS-agnostic HID identity record.

use crate::identity::{format_vid_pid, DeviceKind};

/// Identity and driver-stack facts for one HID / input interface.
#[derive(Debug, Clone)]
pub struct HidDeviceInfo {
    /// Device path (`\\?\hid#...` or `/dev/input/eventN`).
    pub path: String,
    /// USB VID.
    pub vid: u16,
    /// USB PID.
    pub pid: u16,
    /// bcdDevice.
    pub version: u16,
    /// Manufacturer string.
    pub manufacturer: String,
    /// Product string.
    pub product: String,
    /// Serial string, if any.
    pub serial: String,
    /// Classification.
    pub kind: DeviceKind,
    /// Device description.
    pub description: String,
    /// Hardware IDs.
    pub hardware_ids: Vec<String>,
    /// Function driver service.
    pub service: String,
    /// Upper filter drivers, if any.
    pub upper_filters: Vec<String>,
    /// Lower filter drivers, if any.
    pub lower_filters: Vec<String>,
    /// True if the handle needed write access for *read* (Windows).
    pub opened_with_write_access: bool,
}

impl HidDeviceInfo {
    /// One-line listing.
    pub fn summary_line(&self) -> String {
        format!(
            "{}  {}  {}  product='{}'  service='{}'  upper=[{}]  lower=[{}]",
            self.kind.display_name(),
            format_vid_pid(self.vid, self.pid),
            self.path,
            self.product,
            self.service,
            self.upper_filters.join(","),
            self.lower_filters.join(","),
        )
    }
}
