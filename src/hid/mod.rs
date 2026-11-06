//! Read-only HID / input access.
//!
//! Windows: inbox `hid.dll` / `setupapi.dll`.
//! Linux: evdev (`/dev/input/event*`).

mod info;

pub use info::HidDeviceInfo;

#[cfg(windows)]
mod decode;
#[cfg(windows)]
mod device;
#[cfg(windows)]
mod enumerate;
#[cfg(windows)]
mod ffi;

#[cfg(unix)]
mod linux;

#[cfg(windows)]
pub use decode::ReportDecoder;
#[cfg(windows)]
pub use device::{HidReader, RawReport, ReportSource};
#[cfg(windows)]
pub use enumerate::{list_all_hid, list_x52, pick_x52};
#[cfg(windows)]
pub(crate) use ffi::to_wide;

#[cfg(unix)]
pub use linux::{list_all_hid, list_x52, pick_x52, HidReader, RawReport, ReportSource};

#[cfg(windows)]
use crate::error::Error;
#[cfg(windows)]
use crate::pipeline::AxisProcessing;
#[cfg(windows)]
use crate::state::X52State;

#[cfg(windows)]
impl HidReader {
    /// Read one report and decode it. `None` means the wait timed out.
    pub fn read_state(
        &mut self,
        processing: &AxisProcessing,
        report_number: u64,
        elapsed_ms: u64,
    ) -> Result<Option<X52State>, Error> {
        let Some(raw) = self.read_timeout(std::time::Duration::from_millis(250))? else {
            return Ok(None);
        };
        let mut state = self.decode_report(&raw, report_number, elapsed_ms)?;
        state = state.finalize(self.info().kind);
        state.apply_pipeline(processing);
        Ok(Some(state))
    }
}
