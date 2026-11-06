//! Read-only environment diagnose for Linux. Never changes udev or devices.

use std::fs;

use serde::Serialize;

use crate::error::Error;
use crate::identity::DeviceKind;
use crate::linux_input;

/// Snapshot of things that matter for running the X52 on Linux.
#[derive(Debug, Clone, Serialize)]
pub struct DiagnoseReport {
    /// Process is root. Should be false for daily use.
    pub elevated: bool,
    /// `write_path` compile feature.
    pub write_path_compiled: bool,
    /// Memory Integrity is a Windows concept; always `None` here.
    pub memory_integrity: Option<bool>,
    /// Matching X52 devices.
    pub x52_devices: Vec<DeviceStack>,
    /// Other Saitek VID devices.
    pub other_saitek: Vec<DeviceStack>,
}

/// One input device.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceStack {
    /// Kind label.
    pub kind: String,
    /// VID.
    pub vid: u16,
    /// PID.
    pub pid: u16,
    /// Product string.
    pub product: String,
    /// Path (`/dev/input/eventN`).
    pub path: String,
    /// Function driver (evdev).
    pub service: String,
    /// Upper filters (unused on Linux).
    pub upper_filters: Vec<String>,
    /// Lower filters (unused on Linux).
    pub lower_filters: Vec<String>,
    /// Hardware IDs.
    pub hardware_ids: Vec<String>,
}

/// Collect a read-only diagnose report.
pub fn collect() -> Result<DiagnoseReport, Error> {
    let x52 = linux_input::list_x52().unwrap_or_default();
    let x52_devices = x52
        .iter()
        .map(|d| DeviceStack {
            kind: d.kind.display_name().to_string(),
            vid: d.vid,
            pid: d.pid,
            product: d.name.clone(),
            path: d.path.clone(),
            service: "evdev".into(),
            upper_filters: Vec::new(),
            lower_filters: Vec::new(),
            hardware_ids: vec![format!("usb:v{:04X}p{:04X}", d.vid, d.pid)],
        })
        .collect();
    Ok(DiagnoseReport {
        elevated: is_elevated(),
        write_path_compiled: crate::WRITE_PATH_COMPILED,
        memory_integrity: None,
        x52_devices,
        other_saitek: other_saitek_from_sys(),
    })
}

/// True when this process is running as root.
pub fn is_elevated() -> bool {
    unsafe { libc::geteuid() == 0 }
}

fn other_saitek_from_sys() -> Vec<DeviceStack> {
    let Ok(text) = fs::read_to_string("/proc/bus/input/devices") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut name = String::new();
    let mut vid = 0u16;
    let mut pid = 0u16;
    for line in text.lines() {
        if line.starts_with("I:") {
            vid = 0;
            pid = 0;
            name.clear();
            if let Some(rest) = line.split("Vendor=").nth(1) {
                let v = rest.get(..4).unwrap_or("0000");
                vid = u16::from_str_radix(v, 16).unwrap_or(0);
            }
            if let Some(rest) = line.split("Product=").nth(1) {
                let p = rest.get(..4).unwrap_or("0000");
                pid = u16::from_str_radix(p, 16).unwrap_or(0);
            }
        } else if let Some(n) = line.strip_prefix("N: Name=") {
            name = n.trim_matches('"').to_string();
        } else if line.is_empty() {
            let kind = DeviceKind::from_vid_pid(vid, pid);
            if kind == DeviceKind::OtherSaitek {
                out.push(DeviceStack {
                    kind: kind.display_name().into(),
                    vid,
                    pid,
                    product: name.clone(),
                    path: String::new(),
                    service: "evdev".into(),
                    upper_filters: Vec::new(),
                    lower_filters: Vec::new(),
                    hardware_ids: vec![format!("usb:v{vid:04X}p{pid:04X}")],
                });
            }
        }
    }
    out
}

/// Render diagnose as plain text.
pub fn render(report: &DiagnoseReport) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "Privilege          {}\n",
        if report.elevated {
            "root"
        } else {
            "standard user"
        }
    ));
    if report.x52_devices.is_empty() {
        s.push_str("X52                not connected\n");
        s.push_str(
            "Hint               install packaging/linux/99-x52-integrity.rules and replug\n",
        );
    }
    for d in &report.x52_devices {
        s.push_str(&format!(
            "X52                {}  {:04X}:{:04X}\n",
            d.product, d.vid, d.pid
        ));
        s.push_str(&format!("Path               {}\n", d.path));
    }
    for d in &report.other_saitek {
        s.push_str(&format!(
            "Other Saitek       {:04X}:{:04X}  {}\n",
            d.vid, d.pid, d.product
        ));
    }
    s
}
