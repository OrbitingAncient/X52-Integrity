//! Encode documented X52 vendor requests. **No I/O.**
//!
//! USB facts (widely published for Saitek X52 / X52 Pro):
//! - `bmRequestType` = vendor | host-to-device | device (0x40)
//! - `bRequest`      = 0x91
//! - `wValue` / `wIndex` select LED, brightness, MFD text, clock
//!
//! This module only builds the `(request, value, index)` triple. Nothing is
//! sent. The monitor never calls a transport.

/// Documented vendor `bRequest` used by the X52 LED/MFD protocol.
pub const VENDOR_REQUEST: u8 = 0x91;

/// MFD brightness (`wIndex`). `wValue` is 0..=0x80.
pub const IDX_MFD_BRIGHTNESS: u16 = 0xB1;
/// LED brightness (`wIndex`). `wValue` is 0..=0x80.
pub const IDX_LED_BRIGHTNESS: u16 = 0xB2;

/// Dual-color LED identifiers (`wIndex`). `wValue` 0 = off, 1 = on.
/// A color is made by combining the red and green halves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum LedId {
    /// Fire button (single color).
    Fire = 1,
    /// A red half.
    ARed = 2,
    /// A green half.
    AGreen = 3,
    /// B red.
    BRed = 4,
    /// B green.
    BGreen = 5,
    /// D red.
    DRed = 6,
    /// D green.
    DGreen = 7,
    /// E red.
    ERed = 8,
    /// E green.
    EGreen = 9,
    /// T1 red.
    T1Red = 10,
    /// T1 green.
    T1Green = 11,
    /// T2 red.
    T2Red = 12,
    /// T2 green.
    T2Green = 13,
    /// T3 red.
    T3Red = 14,
    /// T3 green.
    T3Green = 15,
    /// POV red.
    PovRed = 16,
    /// POV green.
    PovGreen = 17,
    /// Clutch red.
    ClutchRed = 18,
    /// Clutch green.
    ClutchGreen = 19,
    /// Throttle LED (single color).
    Throttle = 20,
}

/// One vendor control-transfer description. Never executed by this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VendorCommand {
    /// `bRequest`. Always [`VENDOR_REQUEST`] for this device family.
    pub request: u8,
    /// `wValue`.
    pub value: u16,
    /// `wIndex`.
    pub index: u16,
}

impl VendorCommand {
    /// LED on/off.
    pub fn led(id: LedId, on: bool) -> Self {
        Self {
            request: VENDOR_REQUEST,
            value: if on { 1 } else { 0 },
            index: id as u16,
        }
    }

    /// LED brightness 0..=128. Values above 128 are clamped.
    pub fn led_brightness(level: u8) -> Self {
        Self {
            request: VENDOR_REQUEST,
            value: u16::from(level.min(0x80)),
            index: IDX_LED_BRIGHTNESS,
        }
    }

    /// MFD brightness 0..=128. Values above 128 are clamped.
    pub fn mfd_brightness(level: u8) -> Self {
        Self {
            request: VENDOR_REQUEST,
            value: u16::from(level.min(0x80)),
            index: IDX_MFD_BRIGHTNESS,
        }
    }
}

/// Known-safe restore defaults used by a future write-path ExitGuard.
/// Mid brightness, no assumption about LED colors (device default).
pub fn safe_restore_commands() -> [VendorCommand; 2] {
    [
        VendorCommand::led_brightness(0x40),
        VendorCommand::mfd_brightness(0x40),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_is_0x91() {
        let c = VendorCommand::led_brightness(80);
        assert_eq!(c.request, 0x91);
        assert_eq!(c.index, 0xB2);
        assert_eq!(c.value, 80);
    }

    #[test]
    fn brightness_clamped() {
        assert_eq!(VendorCommand::mfd_brightness(255).value, 0x80);
    }

    #[test]
    fn led_off_is_value_zero() {
        let c = VendorCommand::led(LedId::Fire, false);
        assert_eq!(c.value, 0);
        assert_eq!(c.index, 1);
    }
}
