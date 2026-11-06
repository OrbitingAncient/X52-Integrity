//! Virtual joystick output. **Disabled by default.**
//!
//! vJoy is not used in this project (older builds are not HVCI-safe).
//! If a virtual device is added later, it will be **ViGEmBus only**
//! (WHQL, Memory Integrity compatible), still behind SafetyGate, and only
//! after you confirm the install.
//!
//! This module currently ships a null implementation. No bus driver is installed.

use crate::error::Error;
use crate::safety::SafetyGate;
use crate::state::X52State;

/// Sink for processed axes/buttons. The monitor holds [`NullVirtualJoystick`].
pub trait VirtualJoystick {
    /// Push one processed state. Must consult the gate.
    fn update(&self, gate: &SafetyGate, state: &X52State) -> Result<(), Error>;
}

/// Default sink. Always refused.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullVirtualJoystick;

impl VirtualJoystick for NullVirtualJoystick {
    fn update(&self, gate: &SafetyGate, _state: &X52State) -> Result<(), Error> {
        gate.allow_virtual_output("vigembus-virtual-joystick")?;
        unreachable!("SafetyGate returned Ok in monitor-only build");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::safety::SafetyGate;
    use crate::state::{ModeSwitch, X52State};

    #[test]
    fn null_output_refuses() {
        let js = NullVirtualJoystick;
        let gate = SafetyGate::monitor_only();
        let state = X52State {
            report_number: 0,
            elapsed_ms: 0,
            raw: vec![],
            axes: vec![],
            hats: vec![],
            buttons_pressed: vec![],
            button_names: vec![],
            mode: ModeSwitch::Unknown,
        };
        assert!(js.update(&gate, &state).is_err());
    }
}
