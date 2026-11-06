//! Transports that would send [`VendorCommand`]s. Default: never send.

use crate::error::Error;
use crate::safety::SafetyGate;
use crate::vendor::codec::VendorCommand;

/// Sink for vendor commands. The monitor holds a [`NullVendorTransport`].
pub trait VendorTransport {
    /// Send a command if and only if the gate allows it.
    fn send(&self, gate: &SafetyGate, command: VendorCommand) -> Result<(), Error>;
}

/// Default transport. Always returns a safety error. No syscall.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullVendorTransport;

impl VendorTransport for NullVendorTransport {
    fn send(&self, gate: &SafetyGate, command: VendorCommand) -> Result<(), Error> {
        let _ = command;
        gate.allow_vendor_command("vendor-0x91")?;
        unreachable!("SafetyGate::allow_vendor_command returned Ok in monitor-only build");
    }
}

/// Experimental HID feature-report transport.
///
/// **Disabled by default.** Compiled only with `--features write_path`.
/// Even then, [`SafetyGate`] must allow the call, and the X52 may ignore
/// HID feature reports because the historical protocol is a USB vendor
/// control transfer (0x91) rather than a HID SET_REPORT.
///
/// We will not rebind the joystick interface to WinUSB or install a filter
/// driver to force those control transfers. That would be a kernel change.
#[cfg(feature = "write_path")]
pub struct HidFeatureVendorTransport {
    /// Marker so this type is not constructible from the monitor yet.
    _private: (),
}

#[cfg(feature = "write_path")]
impl HidFeatureVendorTransport {
    /// Not used by `x52-monitor`. Construction is the first line of defense.
    pub fn new_unarmed() -> Self {
        Self { _private: () }
    }
}

#[cfg(feature = "write_path")]
impl VendorTransport for HidFeatureVendorTransport {
    fn send(&self, gate: &SafetyGate, command: VendorCommand) -> Result<(), Error> {
        let _ = command;
        gate.allow_vendor_command("hid-feature-vendor-0x91")?;
        Err(Error::Io(
            "write_path compiled, but no HID feature-report send is wired in this deliverable"
                .to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::safety::SafetyGate;
    use crate::vendor::codec::{LedId, VendorCommand};

    #[test]
    fn null_transport_never_sends() {
        let t = NullVendorTransport;
        let gate = SafetyGate::monitor_only();
        let err = t
            .send(&gate, VendorCommand::led(LedId::Fire, true))
            .unwrap_err();
        match err {
            Error::Safety(_) => {}
            other => panic!("expected safety error, got {other}"),
        }
    }
}
