//! Error types. Library code returns these instead of panicking.

use crate::safety::SafetyError;

/// Crate-wide error.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A Win32 API failed. `code` is `GetLastError` / NTSTATUS as available.
    #[error("{api} failed (win32=0x{code:08X} / {code})")]
    Win32 {
        /// API name for logs.
        api: &'static str,
        /// Win32 error or NTSTATUS value.
        code: u32,
    },
    /// No matching X52 HID device is present.
    #[error("no Saitek/Logitech X52 HID device found (VID 06A3, PID 0255/075C/0762)")]
    DeviceNotFound,
    /// The requested device index is out of range.
    #[error("device index {index} is out of range ({count} X52 device(s) found)")]
    DeviceIndexOutOfRange {
        /// Requested index.
        index: usize,
        /// How many matching devices were found.
        count: usize,
    },
    /// HID report decode failed.
    #[error("invalid HID report: {0}")]
    InvalidReport(&'static str),
    /// Profile JSON problem.
    #[error("profile: {0}")]
    Profile(String),
    /// A write was refused by the safety gate.
    #[error("safety gate: {0}")]
    Safety(#[from] SafetyError),
    /// Command-line / usage error.
    #[error("{0}")]
    Usage(String),
    /// Generic I/O.
    #[error("{0}")]
    Io(String),
    /// Driver install/restore needs an elevated process.
    #[error("{0} requires administrator rights")]
    NeedAdmin(&'static str),
    /// Platform-specific failure (Linux udev/evdev, etc.).
    #[error("{0}")]
    Platform(String),
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Error::Io(value.to_string())
    }
}

impl Error {
    /// Capture `GetLastError` immediately after a failed Win32 call.
    #[cfg(windows)]
    pub(crate) fn win32(api: &'static str) -> Self {
        let code = unsafe { windows_sys::Win32::Foundation::GetLastError() };
        Error::Win32 { api, code }
    }
}
