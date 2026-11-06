//! Read-only environment diagnose. Never changes drivers, services, or registry.

use serde::Serialize;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::Security::{
    GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

use crate::error::Error;
use crate::hid::{list_all_hid, list_x52, to_wide, HidDeviceInfo};

/// Snapshot of things that matter for Memory Integrity compatibility.
#[derive(Debug, Clone, Serialize)]
pub struct DiagnoseReport {
    /// Process is elevated. Must be false for this tool.
    pub elevated: bool,
    /// `write_path` compile feature.
    pub write_path_compiled: bool,
    /// Memory Integrity / HVCI enabled, if readable without admin.
    pub memory_integrity: Option<bool>,
    /// Matching X52 devices.
    pub x52_devices: Vec<DeviceStack>,
    /// Other Saitek VID devices (pedals, etc.), not opened for writes.
    pub other_saitek: Vec<DeviceStack>,
}

/// Driver stack for one HID interface.
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
    /// Path.
    pub path: String,
    /// Function driver.
    pub service: String,
    /// Upper filters.
    pub upper_filters: Vec<String>,
    /// Lower filters.
    pub lower_filters: Vec<String>,
    /// Hardware IDs.
    pub hardware_ids: Vec<String>,
}

impl From<&HidDeviceInfo> for DeviceStack {
    fn from(d: &HidDeviceInfo) -> Self {
        Self {
            kind: d.kind.display_name().to_string(),
            vid: d.vid,
            pid: d.pid,
            product: d.product.clone(),
            path: d.path.clone(),
            service: d.service.clone(),
            upper_filters: d.upper_filters.clone(),
            lower_filters: d.lower_filters.clone(),
            hardware_ids: d.hardware_ids.clone(),
        }
    }
}

/// Collect a read-only diagnose report.
pub fn collect() -> Result<DiagnoseReport, Error> {
    let all = list_all_hid()?;
    let x52 = list_x52().unwrap_or_default();
    let other_saitek = all
        .iter()
        .filter(|d| d.kind == crate::identity::DeviceKind::OtherSaitek)
        .map(DeviceStack::from)
        .collect();
    Ok(DiagnoseReport {
        elevated: is_elevated(),
        write_path_compiled: crate::WRITE_PATH_COMPILED,
        memory_integrity: read_memory_integrity(),
        x52_devices: x52.iter().map(DeviceStack::from).collect(),
        other_saitek,
    })
}

/// True when this process token is elevated. The GUI never requests UAC unless
/// the user clicks Install/Remove driver.
pub fn is_elevated() -> bool {
    // Token elevation via OpenProcessToken + GetTokenInformation.
    // If anything fails, report false (we did not request elevation).
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        let ok = OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token);
        if ok == 0 {
            return false;
        }
        let mut elev = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut ret = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elev as *mut TOKEN_ELEVATION as *mut _,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret,
        );
        let _ = CloseHandle(token);
        ok != 0 && elev.TokenIsElevated != 0
    }
}

fn read_memory_integrity() -> Option<bool> {
    // HKLM\SYSTEM\CurrentControlSet\Control\DeviceGuard\Scenarios\HypervisorEnforcedCodeIntegrity
    // Value: Enabled (DWORD). Read-only. Failure → None, never request admin.
    unsafe {
        let path = to_wide(
            r"SYSTEM\CurrentControlSet\Control\DeviceGuard\Scenarios\HypervisorEnforcedCodeIntegrity",
        );
        let mut key: HKEY = std::ptr::null_mut();
        let st = RegOpenKeyExW(HKEY_LOCAL_MACHINE, path.as_ptr(), 0, KEY_READ, &mut key);
        if st != 0 {
            return None;
        }
        let name = to_wide("Enabled");
        let mut dtype = 0u32;
        let mut data = 0u32;
        let mut len = 4u32;
        let st = RegQueryValueExW(
            key,
            name.as_ptr(),
            std::ptr::null_mut(),
            &mut dtype,
            &mut data as *mut u32 as *mut u8,
            &mut len,
        );
        let _ = RegCloseKey(key);
        if st != 0 {
            return None;
        }
        Some(data != 0)
    }
}

/// Render diagnose as plain text.
pub fn render(report: &DiagnoseReport) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "Privilege          {}\n",
        if report.elevated {
            "Administrator"
        } else {
            "Standard user"
        }
    ));
    s.push_str(&format!(
        "Memory Integrity   {}\n",
        match report.memory_integrity {
            Some(true) => "On",
            Some(false) => "Off",
            None => "Unknown",
        }
    ));
    if report.x52_devices.is_empty() {
        s.push_str("X52                not connected\n");
    }
    for d in &report.x52_devices {
        let driver = driver_summary(d);
        s.push_str(&format!(
            "X52                {}  {:04X}:{:04X}\n",
            if d.product.is_empty() {
                d.kind.as_str()
            } else {
                d.product.as_str()
            },
            d.vid,
            d.pid
        ));
        s.push_str(&format!("Driver             {driver}\n"));
        s.push_str(&format!("Path               {}\n", d.path));
    }
    for d in &report.other_saitek {
        s.push_str(&format!(
            "Other Saitek       {:04X}:{:04X}  {}\n",
            d.vid,
            d.pid,
            if d.product.is_empty() {
                "—"
            } else {
                d.product.as_str()
            }
        ));
    }
    s
}

fn driver_summary(d: &DeviceStack) -> String {
    let mut filters = d.upper_filters.clone();
    filters.extend(d.lower_filters.iter().cloned());
    filters.retain(|f| !f.is_empty());
    if !filters.is_empty() {
        return format!("Logitech ({})", filters.join(", "));
    }
    if d.service.is_empty() || d.service.eq_ignore_ascii_case("hidusb") {
        "Windows HID".into()
    } else {
        d.service.clone()
    }
}
