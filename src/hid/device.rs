//! Read-only HID session. Opens for input reports; never sends vendor commands.

use std::time::Duration;

use crate::error::Error;
use crate::hid::decode::ReportDecoder;
use crate::hid::ffi::{self, create_file_read, to_wide, OwnedHandle, Preparsed};
use crate::hid::info::HidDeviceInfo;

/// How the report was obtained. Both paths are HID **reads**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportSource {
    /// Interrupt pipe `ReadFile`.
    Interrupt,
    /// Control-pipe `HidD_GetInputReport` after the interrupt wait timed out.
    GetInputReport,
}

/// One input report as delivered by `ReadFile` or `HidD_GetInputReport`.
#[derive(Debug, Clone)]
pub struct RawReport {
    /// Bytes including report ID at `[0]` when the device uses report IDs.
    pub bytes: Vec<u8>,
    /// Which HID read produced this buffer.
    pub source: ReportSource,
}

/// Open HID device used exclusively for reading.
pub struct HidReader {
    info: HidDeviceInfo,
    handle: OwnedHandle,
    #[allow(dead_code)]
    opened_with_write_access: bool,
    preparsed: Preparsed,
    decoder: ReportDecoder,
    report_len: usize,
    report_id: u8,
    logged_read_error: std::cell::Cell<bool>,
    logged_fallback: std::cell::Cell<bool>,
}

impl HidReader {
    /// Open `info` for reading. Tries GENERIC_READ first.
    pub fn open(info: HidDeviceInfo) -> Result<Self, Error> {
        let path_wide = to_wide(&info.path);
        let mut opened_with_write_access = false;
        // Many HID stacks, including Saitek upper filters, require GENERIC_READ|GENERIC_WRITE
        // even for input reports. We still never send a vendor command on this handle.
        let handle = match create_file_read(&path_wide, true) {
            Ok(h) => {
                opened_with_write_access = true;
                h
            }
            Err(_) => create_file_read(&path_wide, false)?,
        };
        let preparsed = Preparsed::get(handle.as_raw())?;
        let caps = ffi::get_caps(&preparsed)?;
        let report_len = caps.input_report_byte_length as usize;
        if report_len == 0 {
            return Err(Error::InvalidReport("input report length is 0"));
        }
        // Raise the input ring. Do not FlushQueue: this device is change-driven
        // and flushing the only buffered report leaves ReadFile waiting forever.
        let _ = ffi::set_num_input_buffers(handle.as_raw(), 64);
        eprintln!(
            "HID caps: input_report={} bytes  buttons={}  values={}  usage_page=0x{:04X} usage=0x{:04X}",
            report_len,
            caps.number_input_button_caps,
            caps.number_input_value_caps,
            caps.usage_page,
            caps.usage
        );
        let button_caps = ffi::get_button_caps(&preparsed, caps.number_input_button_caps)?;
        let value_caps = ffi::get_value_caps(&preparsed, caps.number_input_value_caps)?;
        let report_id = value_caps
            .iter()
            .map(|c| c.report_id)
            .chain(button_caps.iter().map(|c| c.report_id))
            .find(|&id| id != 0)
            .unwrap_or(0);
        let decoder = ReportDecoder::new(button_caps, value_caps);
        eprintln!("HID report ID byte: {report_id} (0 means the device has no report IDs)");
        Ok(Self {
            info,
            handle,
            opened_with_write_access,
            preparsed,
            decoder,
            report_len,
            report_id,
            logged_read_error: std::cell::Cell::new(false),
            logged_fallback: std::cell::Cell::new(false),
        })
    }

    /// Device identity.
    pub fn info(&self) -> &HidDeviceInfo {
        &self.info
    }

    /// Decoder (HID usage map).
    pub fn decoder(&self) -> &ReportDecoder {
        &self.decoder
    }

    /// Preparsed HID data, needed to decode each report.
    pub fn preparsed(&self) -> &Preparsed {
        &self.preparsed
    }

    /// Input report size in bytes.
    pub fn report_len(&self) -> usize {
        self.report_len
    }

    /// True if the handle was opened with GENERIC_WRITE as a fallback for reading.
    pub fn opened_with_write_access(&self) -> bool {
        self.opened_with_write_access
    }

    /// Blocking overlapped read. `None` on timeout (no report yet).
    ///
    /// If the interrupt pipe yields nothing (common when a vendor upper filter
    /// owns it), fall back to `HidD_GetInputReport`, which is still a HID
    /// **read** via the control pipe — never a vendor 0x91 write.
    pub fn read_timeout(&self, timeout: Duration) -> Result<Option<RawReport>, Error> {
        // Read a little larger than the descriptor length. Some hidclass builds
        // complete only when the buffer is a full USB packet.
        let mut buf = vec![0u8; self.report_len.max(64)];
        let ms = timeout.as_millis().min(u32::MAX as u128) as u32;
        match ffi::read_overlapped(self.handle.as_raw(), &mut buf, ms) {
            Ok(Some(n)) if n > 0 => {
                buf.truncate(n.min(self.report_len.max(n)));
                if buf.len() > self.report_len && self.report_len > 0 {
                    buf.truncate(self.report_len);
                }
                return Ok(Some(RawReport {
                    bytes: buf,
                    source: ReportSource::Interrupt,
                }));
            }
            Ok(_) => {}
            Err(e) => {
                if !self.logged_read_error.replace(true) {
                    eprintln!("interrupt ReadFile: {e}; trying HidD_GetInputReport (still a read)");
                }
            }
        }
        let mut poll = vec![0u8; self.report_len];
        if self.report_id != 0 {
            poll[0] = self.report_id;
        }
        match ffi::get_input_report(self.handle.as_raw(), &mut poll) {
            Ok(_) => {
                if !self.logged_fallback.replace(true) {
                    eprintln!(
                        "interrupt ReadFile returned no bytes; using HidD_GetInputReport (HID read, not vendor 0x91)."
                    );
                }
                Ok(Some(RawReport {
                    bytes: poll,
                    source: ReportSource::GetInputReport,
                }))
            }
            Err(e) => {
                if !self.logged_read_error.replace(true) {
                    eprintln!("HidD_GetInputReport: {e}");
                }
                Ok(None)
            }
        }
    }
}

impl ReportDecoder {
    pub(crate) fn decode_with(
        &self,
        preparsed: &Preparsed,
        report: &[u8],
        report_number: u64,
        elapsed_ms: u64,
    ) -> Result<crate::state::X52State, Error> {
        self.decode_preparsed(preparsed, report, report_number, elapsed_ms)
    }
}

impl HidReader {
    /// Decode using the live preparsed data.
    pub fn decode_report(
        &self,
        raw: &RawReport,
        report_number: u64,
        elapsed_ms: u64,
    ) -> Result<crate::state::X52State, Error> {
        self.decoder
            .decode_with(&self.preparsed, &raw.bytes, report_number, elapsed_ms)
    }
}
