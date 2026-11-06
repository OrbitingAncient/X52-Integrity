//! Software-only response curves, deadzones, and optional ghosting filter.
//! These transform a *copy* of parsed HID state. They never write to the device.

use serde::{Deserialize, Serialize};

use crate::state::AxisValue;

/// Curve shape applied after deadzone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CurveKind {
    /// Pass-through after deadzone.
    #[default]
    Linear,
    /// Smoothstep (Hermite) on the magnitude.
    Smoothstep,
    /// Exponential: `sign * |x|^expo`.
    Expo,
}

/// Per-axis software processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AxisTuning {
    /// Symmetric deadzone in 0.0..=1.0 of full throw (centered axes) or span (unipolar).
    pub deadzone: f64,
    /// Curve.
    pub curve: CurveKind,
    /// Exponent for [`CurveKind::Expo`]. `1.0` is linear. `2.0` is more center-fine.
    pub expo: f64,
    /// Invert the processed value.
    pub invert: bool,
}

impl Default for AxisTuning {
    fn default() -> Self {
        Self {
            deadzone: 0.0,
            curve: CurveKind::Linear,
            expo: 1.5,
            invert: false,
        }
    }
}

/// Button ghosting / debounce in software.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GhostingFilter {
    /// Master enable. Default off.
    pub enabled: bool,
    /// Consecutive reports a button must stay down before we report it.
    pub press_reports: u32,
    /// Consecutive reports a button must stay up before we release it.
    pub release_reports: u32,
}

impl Default for GhostingFilter {
    fn default() -> Self {
        Self {
            enabled: false,
            press_reports: 2,
            release_reports: 2,
        }
    }
}

/// Bundle of software processing applied to a state snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AxisProcessing {
    /// Default tuning used when an axis has no override.
    pub default: AxisTuning,
    /// Named overrides (`X`, `Y`, `Rz`, `Slider`, …).
    pub axes: std::collections::BTreeMap<String, AxisTuning>,
    /// Optional software debounce.
    pub ghosting: GhostingFilter,
}

impl Default for AxisProcessing {
    fn default() -> Self {
        Self {
            default: AxisTuning::default(),
            axes: std::collections::BTreeMap::new(),
            ghosting: GhostingFilter::default(),
        }
    }
}

impl AxisProcessing {
    /// Process one axis. Output stays in the same domain as `normalized`.
    pub fn apply(&self, axis: &AxisValue) -> f64 {
        let tuning = self.axes.get(&axis.name).unwrap_or(&self.default);
        apply_tuning(axis.normalized, axis.centered, tuning)
    }
}

/// Apply deadzone + curve + invert to a normalized value.
pub fn apply_tuning(normalized: f64, centered: bool, tuning: &AxisTuning) -> f64 {
    let mut v = if centered {
        apply_centered_deadzone(normalized, tuning.deadzone)
    } else {
        apply_unipolar_deadzone(normalized, tuning.deadzone)
    };
    v = apply_curve(v, centered, tuning.curve, tuning.expo);
    if tuning.invert {
        v = if centered { -v } else { 1.0 - v };
    }
    v
}

fn apply_centered_deadzone(v: f64, dz: f64) -> f64 {
    let dz = dz.clamp(0.0, 0.95);
    let a = v.abs();
    if a <= dz {
        0.0
    } else {
        v.signum() * (a - dz) / (1.0 - dz)
    }
}

fn apply_unipolar_deadzone(v: f64, dz: f64) -> f64 {
    let dz = dz.clamp(0.0, 0.95);
    if v <= dz {
        0.0
    } else {
        (v - dz) / (1.0 - dz)
    }
}

fn apply_curve(v: f64, centered: bool, kind: CurveKind, expo: f64) -> f64 {
    match kind {
        CurveKind::Linear => v,
        CurveKind::Smoothstep => {
            if centered {
                let a = v.abs().clamp(0.0, 1.0);
                let s = a * a * (3.0 - 2.0 * a);
                v.signum() * s
            } else {
                let x = v.clamp(0.0, 1.0);
                x * x * (3.0 - 2.0 * x)
            }
        }
        CurveKind::Expo => {
            let e = if expo.is_finite() && expo > 0.0 {
                expo
            } else {
                1.0
            };
            if centered {
                v.signum() * v.abs().powf(e)
            } else {
                v.clamp(0.0, 1.0).powf(e)
            }
        }
    }
}

/// Normalize a raw HID value.
pub fn normalize(raw: u32, logical_min: i32, logical_max: i32, centered: bool) -> f64 {
    let min = logical_min as f64;
    let max = logical_max as f64;
    if max <= min {
        return if centered { 0.0 } else { 0.0 };
    }
    let r = (raw as f64).clamp(min, max);
    if centered {
        let mid = (min + max) / 2.0;
        let half = (max - min) / 2.0;
        if half == 0.0 {
            0.0
        } else {
            ((r - mid) / half).clamp(-1.0, 1.0)
        }
    } else {
        ((r - min) / (max - min)).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_deadzone_zeros_the_middle() {
        let t = AxisTuning {
            deadzone: 0.1,
            ..AxisTuning::default()
        };
        assert_eq!(apply_tuning(0.0, true, &t), 0.0);
        assert_eq!(apply_tuning(0.05, true, &t), 0.0);
        let v = apply_tuning(1.0, true, &t);
        assert!((v - 1.0).abs() < 1e-9);
    }

    #[test]
    fn invert_centered() {
        let t = AxisTuning {
            invert: true,
            ..AxisTuning::default()
        };
        assert!((apply_tuning(0.5, true, &t) + 0.5).abs() < 1e-9);
    }

    #[test]
    fn normalize_centered_midpoint() {
        let n = normalize(512, 0, 1024, true);
        assert!(n.abs() < 1e-9);
        assert!((normalize(1024, 0, 1024, true) - 1.0).abs() < 1e-9);
        assert!((normalize(0, 0, 1024, true) + 1.0).abs() < 1e-9);
    }

    #[test]
    fn unipolar_throttle() {
        assert!((normalize(0, 0, 255, false) - 0.0).abs() < 1e-9);
        assert!((normalize(255, 0, 255, false) - 1.0).abs() < 1e-9);
    }
}
