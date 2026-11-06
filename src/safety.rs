//! Runtime safety gate. Writes are denied unless every latch is open.
//!
//! The monitor binary constructs [`SafetyGate::monitor_only`] and never calls
//! an arming method. Arming methods exist only so the optional write path can
//! be reviewed; they still require the `write_path` compile feature.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

/// How this process is allowed to talk to hardware and virtual devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    /// Read HID reports and display them. No vendor transfers, no virtual
    /// joystick writes, no keyboard injection.
    MonitorOnly,
}

/// Why a write was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafetyError {
    /// Process is in monitor-only mode.
    MonitorOnly {
        /// What the caller tried to do.
        action: &'static str,
    },
    /// `write_path` feature is not compiled into this binary.
    WritePathNotCompiled {
        /// What the caller tried to do.
        action: &'static str,
    },
    /// Feature is compiled but the corresponding latch is not armed.
    NotArmed {
        /// What the caller tried to do.
        action: &'static str,
    },
    /// Armed, but the user has not confirmed this specific apply step.
    ApplyNotConfirmed {
        /// What the caller tried to do.
        action: &'static str,
    },
}

impl fmt::Display for SafetyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SafetyError::MonitorOnly { action } => {
                write!(f, "refused {action}: process is MONITOR-ONLY")
            }
            SafetyError::WritePathNotCompiled { action } => {
                write!(f, "refused {action}: write_path feature is not compiled")
            }
            SafetyError::NotArmed { action } => {
                write!(f, "refused {action}: safety latch is not armed")
            }
            SafetyError::ApplyNotConfirmed { action } => {
                write!(f, "refused {action}: no explicit apply confirmation")
            }
        }
    }
}

impl std::error::Error for SafetyError {}

/// Process-wide write policy. Cheap to clone (atomics behind the latches).
///
/// Construction is the only way to get a gate. The monitor uses
/// [`SafetyGate::monitor_only`], which has no public arming API.
#[derive(Debug)]
pub struct SafetyGate {
    mode: RunMode,
    vendor_armed: AtomicBool,
    virtual_output_armed: AtomicBool,
    keyboard_armed: AtomicBool,
    apply_confirmed: AtomicBool,
}

impl SafetyGate {
    /// Default, and the only constructor used by `x52-monitor`.
    pub fn monitor_only() -> Self {
        Self {
            mode: RunMode::MonitorOnly,
            vendor_armed: AtomicBool::new(false),
            virtual_output_armed: AtomicBool::new(false),
            keyboard_armed: AtomicBool::new(false),
            apply_confirmed: AtomicBool::new(false),
        }
    }

    /// Current run mode.
    pub fn mode(&self) -> RunMode {
        self.mode
    }

    /// Snapshot of latches for the UI.
    pub fn snapshot(&self) -> GateSnapshot {
        GateSnapshot {
            mode: self.mode,
            write_path_compiled: crate::WRITE_PATH_COMPILED,
            vendor_armed: self.vendor_armed.load(Ordering::SeqCst),
            virtual_output_armed: self.virtual_output_armed.load(Ordering::SeqCst),
            keyboard_armed: self.keyboard_armed.load(Ordering::SeqCst),
            apply_confirmed: self.apply_confirmed.load(Ordering::SeqCst),
        }
    }

    /// Permit a vendor 0x91 command. Always fails in this deliverable.
    pub fn allow_vendor_command(&self, action: &'static str) -> Result<(), SafetyError> {
        self.deny(action)
    }

    /// Permit a virtual-joystick update. Always fails in this deliverable.
    pub fn allow_virtual_output(&self, action: &'static str) -> Result<(), SafetyError> {
        self.deny(action)
    }

    /// Permit SendInput / macro injection. Always fails in this deliverable.
    pub fn allow_keyboard_injection(&self, action: &'static str) -> Result<(), SafetyError> {
        self.deny(action)
    }

    fn deny(&self, action: &'static str) -> Result<(), SafetyError> {
        if !crate::WRITE_PATH_COMPILED {
            return Err(SafetyError::WritePathNotCompiled { action });
        }
        match self.mode {
            RunMode::MonitorOnly => Err(SafetyError::MonitorOnly { action }),
        }
    }
}

/// Serializable view of the gate, for the UI and `--json` snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateSnapshot {
    /// Run mode.
    pub mode: RunMode,
    /// Whether send symbols exist in this binary.
    pub write_path_compiled: bool,
    /// Vendor latch.
    pub vendor_armed: bool,
    /// Virtual joystick latch.
    pub virtual_output_armed: bool,
    /// Keyboard/macro latch.
    pub keyboard_armed: bool,
    /// Per-apply confirmation latch.
    pub apply_confirmed: bool,
}

impl GateSnapshot {
    /// One-line summary for the dashboard.
    pub fn line(&self) -> String {
        format!(
            "vendor={}  vigem={}  inject={}  apply={}  write_path={}",
            blocked(self.vendor_armed),
            blocked(self.virtual_output_armed),
            blocked(self.keyboard_armed),
            if self.apply_confirmed { "YES" } else { "NO" },
            if self.write_path_compiled {
                "compiled"
            } else {
                "absent"
            },
        )
    }
}

fn blocked(armed: bool) -> &'static str {
    if armed {
        "ARMED"
    } else {
        "BLOCKED"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_only_blocks_every_write() {
        let gate = SafetyGate::monitor_only();
        assert!(gate.allow_vendor_command("led").is_err());
        assert!(gate.allow_virtual_output("axis").is_err());
        assert!(gate.allow_keyboard_injection("macro").is_err());
        let snap = gate.snapshot();
        assert!(!snap.vendor_armed);
        assert!(!snap.virtual_output_armed);
        assert!(!snap.keyboard_armed);
        assert!(!snap.apply_confirmed);
        assert_eq!(snap.mode, RunMode::MonitorOnly);
    }

    #[test]
    fn default_build_reports_write_path_absent() {
        assert!(!crate::WRITE_PATH_COMPILED);
        let err = SafetyGate::monitor_only()
            .allow_vendor_command("mfd")
            .unwrap_err();
        assert!(matches!(err, SafetyError::WritePathNotCompiled { .. }));
    }
}
