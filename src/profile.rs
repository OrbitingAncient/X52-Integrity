//! JSON profiles. Safety flags default to the safest value when omitted.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::pipeline::{AxisProcessing, AxisTuning, CurveKind, GhostingFilter};

/// On-disk profile. Unknown fields are ignored; missing safety flags default off.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    /// Schema version. Currently 1.
    pub schema_version: u32,
    /// Display name.
    pub name: String,
    /// Notes.
    pub description: String,
    /// Safety latches. Defaults are all-deny / monitor-only.
    pub safety: ProfileSafety,
    /// Optional device filter.
    pub device: DeviceFilter,
    /// Software axis processing.
    pub processing: AxisProcessing,
    /// Optional auto-load by foreground process name. Never auto-applies macros.
    pub auto_load: AutoLoad,
    /// Keyboard / macro bindings. Present in the schema, ignored by the monitor.
    pub bindings: Vec<Binding>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            schema_version: 1,
            name: "monitor-only".to_string(),
            description: "Safe default profile. No hardware writes, no injection.".to_string(),
            safety: ProfileSafety::default(),
            device: DeviceFilter::default(),
            processing: AxisProcessing::default(),
            auto_load: AutoLoad::default(),
            bindings: Vec::new(),
        }
    }
}

/// Safety flags stored in the profile. The running process still has the final say.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileSafety {
    /// If true (default), the process stays in monitor-only regardless of other flags.
    pub monitor_only: bool,
    /// Allow vendor 0x91 LED/MFD commands. Default false. Still needs a runtime arm.
    pub allow_vendor_commands: bool,
    /// Allow virtual joystick output. Default false.
    pub allow_virtual_joystick_output: bool,
    /// Allow keyboard injection. Default false.
    pub allow_keyboard_injection: bool,
    /// Allow macros. Default false.
    pub allow_macros: bool,
    /// Require an interactive apply before any write. Default true.
    pub require_apply_confirmation: bool,
}

impl Default for ProfileSafety {
    fn default() -> Self {
        Self {
            monitor_only: true,
            allow_vendor_commands: false,
            allow_virtual_joystick_output: false,
            allow_keyboard_injection: false,
            allow_macros: false,
            require_apply_confirmation: true,
        }
    }
}

/// Match a physical device.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DeviceFilter {
    /// USB VID. Default 0x06A3.
    pub vid: u16,
    /// USB PID, or null to accept any X52 PID.
    pub pid: Option<u16>,
    /// Substring of the product string, if set.
    pub product_match: Option<String>,
}

impl Default for DeviceFilter {
    fn default() -> Self {
        Self {
            vid: crate::identity::SAITEK_VID,
            pid: None,
            product_match: None,
        }
    }
}

/// Auto-load this profile when a process name is in the foreground.
/// The monitor logs a match; it never auto-enables writes or macros.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoLoad {
    /// Master enable. Default false.
    pub enabled: bool,
    /// Executable name, e.g. `dcs.exe`.
    pub process_name: Option<String>,
}

impl Default for AutoLoad {
    fn default() -> Self {
        Self {
            enabled: false,
            process_name: None,
        }
    }
}

/// Placeholder binding. Not executed by the monitor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Binding {
    /// Button usage or name.
    pub source: String,
    /// Intended action (documented only).
    pub action: String,
    /// Must stay false for this deliverable.
    pub enabled: bool,
}

impl Default for Binding {
    fn default() -> Self {
        Self {
            source: String::new(),
            action: String::new(),
            enabled: false,
        }
    }
}

impl Profile {
    /// Load from a JSON file.
    pub fn load(path: &Path) -> Result<Self, Error> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| Error::Profile(format!("read {}: {e}", path.display())))?;
        Self::from_json(&text)
    }

    /// Parse JSON text.
    pub fn from_json(text: &str) -> Result<Self, Error> {
        let profile: Profile =
            serde_json::from_str(text).map_err(|e| Error::Profile(format!("parse: {e}")))?;
        profile.validate()?;
        Ok(profile)
    }

    /// Serialize pretty JSON.
    pub fn to_json_pretty(&self) -> Result<String, Error> {
        serde_json::to_string_pretty(self).map_err(|e| Error::Profile(format!("serialize: {e}")))
    }

    /// Reject profiles that ask the monitor to do something it will not do
    /// without warning. We still load them, but `validate` records issues.
    pub fn validate(&self) -> Result<(), Error> {
        if self.schema_version != 1 {
            return Err(Error::Profile(format!(
                "unsupported schema_version {}",
                self.schema_version
            )));
        }
        Ok(())
    }

    /// True when this profile requests any write capability.
    /// The monitor logs a warning and ignores those flags.
    pub fn requests_writes(&self) -> bool {
        !self.safety.monitor_only
            || self.safety.allow_vendor_commands
            || self.safety.allow_virtual_joystick_output
            || self.safety.allow_keyboard_injection
            || self.safety.allow_macros
            || self.bindings.iter().any(|b| b.enabled)
    }
}

/// Built-in example used when `--profile` is omitted.
pub fn example_monitor_only() -> Profile {
    let mut processing = AxisProcessing::default();
    processing.default = AxisTuning {
        deadzone: 0.02,
        curve: CurveKind::Linear,
        expo: 1.5,
        invert: false,
    };
    processing.axes.insert(
        "X".to_string(),
        AxisTuning {
            deadzone: 0.03,
            curve: CurveKind::Expo,
            expo: 1.6,
            invert: false,
        },
    );
    processing.axes.insert(
        "Y".to_string(),
        AxisTuning {
            deadzone: 0.03,
            curve: CurveKind::Expo,
            expo: 1.6,
            invert: false,
        },
    );
    processing.axes.insert(
        "Slider".to_string(),
        AxisTuning {
            deadzone: 0.01,
            curve: CurveKind::Linear,
            expo: 1.0,
            invert: false,
        },
    );
    processing.ghosting = GhostingFilter {
        enabled: false,
        press_reports: 2,
        release_reports: 2,
    };
    Profile {
        schema_version: 1,
        name: "example-monitor-only".to_string(),
        description: "Ships with the repo. All write flags false. Safe to load.".to_string(),
        safety: ProfileSafety::default(),
        device: DeviceFilter::default(),
        processing,
        auto_load: AutoLoad::default(),
        bindings: vec![Binding {
            source: "B01 Trigger".to_string(),
            action: "documented-only: would fire a weapon mapping in a future write-path build"
                .to_string(),
            enabled: false,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omitted_safety_flags_are_deny() {
        let json = r#"{"schema_version":1,"name":"t"}"#;
        let p = Profile::from_json(json).unwrap();
        assert!(p.safety.monitor_only);
        assert!(!p.safety.allow_vendor_commands);
        assert!(!p.safety.allow_virtual_joystick_output);
        assert!(!p.safety.allow_keyboard_injection);
        assert!(!p.safety.allow_macros);
        assert!(p.safety.require_apply_confirmation);
        assert!(!p.requests_writes());
    }

    #[test]
    fn write_request_is_detected_and_still_parses() {
        let json = r#"{
            "schema_version": 1,
            "name": "unsafe-looking",
            "safety": { "monitor_only": false, "allow_vendor_commands": true }
        }"#;
        let p = Profile::from_json(json).unwrap();
        assert!(p.requests_writes());
    }

    #[test]
    fn example_roundtrip() {
        let p = example_monitor_only();
        let text = p.to_json_pretty().unwrap();
        let q = Profile::from_json(&text).unwrap();
        assert_eq!(p.name, q.name);
        assert!(!q.requests_writes());
    }

    #[test]
    fn shipped_example_file_loads() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("profiles")
            .join("example-monitor-only.json");
        let p = Profile::load(&path).unwrap();
        assert!(p.safety.monitor_only);
        assert!(!p.requests_writes());
    }
}
