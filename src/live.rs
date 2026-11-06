//! Cross-platform live joystick state.
//!
//! Windows uses the inbox `winmm` API (HVCI-safe). Linux uses evdev.

use crate::error::Error;
use crate::identity::DeviceKind;
use crate::state::X52State;

/// One live input source.
#[derive(Debug, Clone)]
pub struct LiveDevice {
    /// OS identifier (winmm id or event node).
    pub id: u32,
    /// USB VID.
    pub vid: u16,
    /// USB PID.
    pub pid: u16,
    /// Product name.
    pub name: String,
    /// Axes advertised.
    pub num_axes: u32,
    /// Buttons advertised.
    pub num_buttons: u32,
    /// Kind.
    pub kind: DeviceKind,
    /// Device path when the OS exposes one.
    pub path: String,
}

/// X52 devices the OS currently exposes as a game controller.
pub fn list_x52() -> Result<Vec<LiveDevice>, Error> {
    #[cfg(windows)]
    {
        Ok(crate::winmm::list_x52()?
            .into_iter()
            .map(|d| LiveDevice {
                id: d.id,
                vid: d.vid,
                pid: d.pid,
                name: d.name,
                num_axes: d.num_axes,
                num_buttons: d.num_buttons,
                kind: d.kind,
                path: String::new(),
            })
            .collect())
    }
    #[cfg(unix)]
    {
        Ok(crate::linux_input::list_x52()?
            .into_iter()
            .map(|d| LiveDevice {
                id: 0,
                vid: d.vid,
                pid: d.pid,
                name: d.name,
                num_axes: d.num_axes,
                num_buttons: d.num_buttons,
                kind: d.kind,
                path: d.path,
            })
            .collect())
    }
    #[cfg(not(any(windows, unix)))]
    {
        Ok(Vec::new())
    }
}

/// Read one live snapshot.
pub fn read_state(
    dev: &LiveDevice,
    report_number: u64,
    elapsed_ms: u64,
) -> Result<X52State, Error> {
    #[cfg(windows)]
    {
        let w = crate::winmm::WinmmDevice {
            id: dev.id,
            vid: dev.vid,
            pid: dev.pid,
            name: dev.name.clone(),
            num_axes: dev.num_axes,
            num_buttons: dev.num_buttons,
            kind: dev.kind,
        };
        crate::winmm::read_state(&w, report_number, elapsed_ms)
    }
    #[cfg(unix)]
    {
        let d = crate::linux_input::LiveDevice {
            path: dev.path.clone(),
            vid: dev.vid,
            pid: dev.pid,
            name: dev.name.clone(),
            num_axes: dev.num_axes,
            num_buttons: dev.num_buttons,
            kind: dev.kind,
        };
        crate::linux_input::read_state(&d, report_number, elapsed_ms)
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = (dev, report_number, elapsed_ms);
        Err(Error::Platform(
            "live input is not supported on this OS".into(),
        ))
    }
}
