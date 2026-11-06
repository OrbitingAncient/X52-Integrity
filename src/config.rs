//! Persistent per-user config, including first-run state.

use std::fs;

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::paths;

/// User preferences stored next to the install.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Schema version.
    #[serde(default = "default_schema")]
    pub schema: u32,
    /// Set after the first-run panel is dismissed.
    #[serde(default)]
    pub first_run_complete: bool,
    /// Last live axis snapshot used as a coarse calibration reference.
    #[serde(default)]
    pub calibration: Option<CalibrationSnapshot>,
}

/// Center/idle values captured from a first-run helper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationSnapshot {
    /// Stick X raw.
    pub x: u32,
    /// Stick Y raw.
    pub y: u32,
    /// Throttle raw.
    pub z: u32,
    /// Twist raw.
    pub r: u32,
}

fn default_schema() -> u32 {
    1
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema: 1,
            first_run_complete: false,
            calibration: None,
        }
    }
}

impl AppConfig {
    /// Load from disk, or defaults if missing.
    pub fn load() -> Self {
        let path = paths::config_path();
        let Ok(text) = fs::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    /// Write to disk.
    pub fn save(&self) -> Result<(), Error> {
        let path = paths::config_path();
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| Error::Io(e.to_string()))?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, text)?;
        fs::rename(&tmp, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_first_run() {
        let c = AppConfig::default();
        assert!(!c.first_run_complete);
        assert_eq!(c.schema, 1);
    }
}
