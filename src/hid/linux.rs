//! Linux HID/evdev backend. Read-only.

use std::time::Duration;

use crate::error::Error;
use crate::hid::info::HidDeviceInfo;
use crate::identity::DeviceKind;
use crate::linux_input;
use crate::pipeline::AxisProcessing;
use crate::state::X52State;

/// How the report was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportSource {
    /// evdev absolute/key state.
    Evdev,
}

/// Raw bytes are not used on the Linux path.
#[derive(Debug, Clone)]
pub struct RawReport {
    /// Unused on Linux.
    pub bytes: Vec<u8>,
    /// Source.
    pub source: ReportSource,
}

/// Open evdev node used exclusively for reading.
pub struct HidReader {
    info: HidDeviceInfo,
    live: linux_input::LiveDevice,
}

impl HidReader {
    /// Open `info` for reading.
    pub fn open(info: HidDeviceInfo) -> Result<Self, Error> {
        let live = linux_input::LiveDevice {
            path: info.path.clone(),
            vid: info.vid,
            pid: info.pid,
            name: info.product.clone(),
            num_axes: 6,
            num_buttons: 32,
            kind: info.kind,
        };
        Ok(Self { info, live })
    }

    /// Device identity.
    pub fn info(&self) -> &HidDeviceInfo {
        &self.info
    }

    /// Always false on Linux.
    pub fn opened_with_write_access(&self) -> bool {
        false
    }

    /// Read one state snapshot.
    pub fn read_state(
        &mut self,
        processing: &AxisProcessing,
        report_number: u64,
        elapsed_ms: u64,
    ) -> Result<Option<X52State>, Error> {
        let _ = Duration::from_millis(0);
        let mut state = linux_input::read_state(&self.live, report_number, elapsed_ms)?;
        state.apply_pipeline(processing);
        Ok(Some(state))
    }

    /// Compatibility with the Windows reader.
    pub fn read_timeout(&self, _timeout: Duration) -> Result<Option<RawReport>, Error> {
        Ok(None)
    }
}

/// Every input device we can classify.
pub fn list_all_hid() -> Result<Vec<HidDeviceInfo>, Error> {
    list_x52()
}

/// X52 devices.
pub fn list_x52() -> Result<Vec<HidDeviceInfo>, Error> {
    Ok(linux_input::list_x52()?
        .into_iter()
        .map(|d| HidDeviceInfo {
            path: d.path,
            vid: d.vid,
            pid: d.pid,
            version: 0,
            manufacturer: "Saitek".into(),
            product: d.name,
            serial: String::new(),
            kind: d.kind,
            description: d.kind.display_name().into(),
            hardware_ids: vec![format!("usb:v{:04X}p{:04X}", d.vid, d.pid)],
            service: "evdev".into(),
            upper_filters: Vec::new(),
            lower_filters: Vec::new(),
            opened_with_write_access: false,
        })
        .collect())
}

/// Pick one X52 by index.
pub fn pick_x52(index: usize) -> Result<HidDeviceInfo, Error> {
    let list = list_x52()?;
    list.into_iter().nth(index).ok_or_else(|| {
        let count = linux_input::list_x52().map(|v| v.len()).unwrap_or(0);
        if count == 0 {
            Error::DeviceNotFound
        } else {
            Error::DeviceIndexOutOfRange { index, count }
        }
    })
}

/// Unused on Linux; kept so `DeviceKind` import stays meaningful.
#[allow(dead_code)]
fn _kind() -> DeviceKind {
    DeviceKind::X52
}
