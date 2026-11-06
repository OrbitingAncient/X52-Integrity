//! Parsed X52 input snapshot. Pure data; no I/O.

use serde::Serialize;

use crate::identity::DeviceKind;
use crate::map::button_name;
use crate::pipeline::AxisProcessing;
use crate::safety::GateSnapshot;

/// One decoded HID axis.
#[derive(Debug, Clone, Serialize)]
pub struct AxisValue {
    /// HID usage page.
    pub usage_page: u16,
    /// HID usage (e.g. 0x30 = X).
    pub usage: u16,
    /// Friendly label when known (`X`, `Y`, `Throttle`, …).
    pub name: String,
    /// Raw HID value.
    pub raw: u32,
    /// Logical min from the report descriptor.
    pub logical_min: i32,
    /// Logical max from the report descriptor.
    pub logical_max: i32,
    /// Normalized to `-1.0..=1.0` (centered) or `0.0..=1.0` (unipolar).
    pub normalized: f64,
    /// After software deadzone/curve (still CPU-only).
    pub processed: f64,
    /// True for stick X/Y/twist/rotaries; false for throttle/slider.
    pub centered: bool,
}

/// Hat / POV switch.
#[derive(Debug, Clone, Serialize)]
pub struct HatValue {
    /// HID usage page.
    pub usage_page: u16,
    /// HID usage (0x39 = Hat switch).
    pub usage: u16,
    /// Friendly label.
    pub name: String,
    /// Raw value.
    pub raw: u32,
    /// Logical min.
    pub logical_min: i32,
    /// Logical max.
    pub logical_max: i32,
    /// True when the descriptor says the control is null (idle).
    pub idle: bool,
    /// Compass label (`N`, `NE`, …) or `idle`.
    pub direction: String,
}

/// Mode rotary on the stick (M1 / M2 / M3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ModeSwitch {
    /// Red / mode 1.
    M1,
    /// Purple / mode 2.
    M2,
    /// Blue / mode 3.
    M3,
    /// Could not infer from buttons.
    Unknown,
}

impl ModeSwitch {
    /// Short label.
    pub fn as_str(self) -> &'static str {
        match self {
            ModeSwitch::M1 => "M1",
            ModeSwitch::M2 => "M2",
            ModeSwitch::M3 => "M3",
            ModeSwitch::Unknown => "unknown",
        }
    }
}

/// One complete parsed report.
#[derive(Debug, Clone, Serialize)]
pub struct X52State {
    /// Monotonic report counter for this session.
    pub report_number: u64,
    /// Milliseconds since the monitor session started.
    pub elapsed_ms: u64,
    /// Raw input report bytes (includes report ID if the device uses one).
    pub raw: Vec<u8>,
    /// Decoded axes.
    pub axes: Vec<AxisValue>,
    /// Decoded hats.
    pub hats: Vec<HatValue>,
    /// HID button usages that are currently down (1-based).
    pub buttons_pressed: Vec<u16>,
    /// Friendly names parallel to `buttons_pressed`.
    pub button_names: Vec<String>,
    /// Inferred mode rotary.
    pub mode: ModeSwitch,
}

impl X52State {
    /// Attach friendly button names and infer the mode rotary.
    pub fn finalize(mut self, kind: DeviceKind) -> Self {
        self.button_names = self
            .buttons_pressed
            .iter()
            .map(|b| button_name(kind, *b).to_string())
            .collect();
        self.mode = infer_mode(&self.buttons_pressed, kind);
        self
    }

    /// Apply software axis processing in place. Never writes to the device.
    pub fn apply_pipeline(&mut self, proc: &AxisProcessing) {
        for axis in &mut self.axes {
            axis.processed = proc.apply(axis);
        }
    }
}

fn infer_mode(pressed: &[u16], kind: DeviceKind) -> ModeSwitch {
    // Community overlay: buttons 27/28/29 are M1/M2/M3 on many X52 Pro maps.
    // HID usages remain authoritative; this is display-only.
    let _ = kind;
    if pressed.contains(&27) {
        ModeSwitch::M1
    } else if pressed.contains(&28) {
        ModeSwitch::M2
    } else if pressed.contains(&29) {
        ModeSwitch::M3
    } else {
        ModeSwitch::Unknown
    }
}

/// JSON-friendly snapshot including safety posture (no secrets).
#[derive(Debug, Clone, Serialize)]
pub struct MonitorSnapshot {
    /// Build banner.
    pub banner: String,
    /// Device product string.
    pub product: String,
    /// VID.
    pub vid: u16,
    /// PID.
    pub pid: u16,
    /// Kind.
    pub kind: String,
    /// HID path.
    pub path: String,
    /// Driver stack (read-only diagnose).
    pub driver_service: String,
    /// Upper filters, if any.
    pub upper_filters: Vec<String>,
    /// Lower filters, if any.
    pub lower_filters: Vec<String>,
    /// Safety gate.
    pub safety: SafetySnapshot,
    /// Latest parsed state, if a report has been read.
    pub state: Option<X52State>,
}

/// Serde view of the gate.
#[derive(Debug, Clone, Serialize)]
pub struct SafetySnapshot {
    /// `MONITOR-ONLY`.
    pub mode: String,
    /// Compile-time write path.
    pub write_path_compiled: bool,
    /// Vendor latch.
    pub vendor_armed: bool,
    /// Virtual joystick latch.
    pub virtual_output_armed: bool,
    /// Keyboard latch.
    pub keyboard_armed: bool,
}

impl From<GateSnapshot> for SafetySnapshot {
    fn from(g: GateSnapshot) -> Self {
        Self {
            mode: match g.mode {
                crate::safety::RunMode::MonitorOnly => "MONITOR-ONLY".to_string(),
            },
            write_path_compiled: g.write_path_compiled,
            vendor_armed: g.vendor_armed,
            virtual_output_armed: g.virtual_output_armed,
            keyboard_armed: g.keyboard_armed,
        }
    }
}
