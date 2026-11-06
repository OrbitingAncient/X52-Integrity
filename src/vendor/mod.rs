//! Optional vendor-command layer (LED / MFD).
//!
//! Isolated from the read path. The codec is always compiled (pure data).
//! Sending is behind the safety gate and, for the actual HID feature-report
//! transport, behind the `write_path` compile feature.

pub mod codec;
pub mod transport;

pub use codec::VendorCommand;
pub use transport::{NullVendorTransport, VendorTransport};
