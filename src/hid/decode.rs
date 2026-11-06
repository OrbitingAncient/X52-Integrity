//! Turn a HID input report into [`X52State`] using `HidP_*`.

use std::collections::BTreeMap;

use crate::error::Error;
use crate::hid::ffi::{self, HidpButtonCaps, HidpValueCaps, Preparsed};
use crate::map::{
    axis_is_centered, axis_name, hat_direction, USAGE_PAGE_BUTTON, USAGE_PAGE_GENERIC_DESKTOP,
};
use crate::pipeline::normalize;
use crate::state::{AxisValue, HatValue, ModeSwitch, X52State};

/// Cached button/value caps from the report descriptor.
pub struct ReportDecoder {
    button_caps: Vec<HidpButtonCaps>,
    value_caps: Vec<HidpValueCaps>,
}

impl ReportDecoder {
    pub(crate) fn new(button_caps: Vec<HidpButtonCaps>, value_caps: Vec<HidpValueCaps>) -> Self {
        Self {
            button_caps,
            value_caps,
        }
    }

    pub(crate) fn decode_preparsed(
        &self,
        preparsed: &Preparsed,
        report: &[u8],
        report_number: u64,
        elapsed_ms: u64,
    ) -> Result<X52State, Error> {
        if report.is_empty() {
            return Err(Error::InvalidReport("empty report"));
        }
        let mut owned = report.to_vec();

        let mut buttons = Vec::new();
        let mut pages = Vec::new();
        for cap in &self.button_caps {
            if !pages.contains(&cap.usage_page) {
                pages.push(cap.usage_page);
            }
        }
        if pages.is_empty() {
            pages.push(USAGE_PAGE_BUTTON);
        }
        for page in pages {
            match ffi::get_usages(preparsed, page, &mut owned) {
                Ok(list) => buttons.extend(list),
                Err(_) => {}
            }
        }
        buttons.sort_unstable();
        buttons.dedup();

        let mut axes = Vec::new();
        let mut hats = Vec::new();
        let mut same_name_count: BTreeMap<String, usize> = BTreeMap::new();

        for cap in &self.value_caps {
            let usages = if cap.is_range != 0 {
                (cap.usage_min..=cap.usage_max).collect::<Vec<_>>()
            } else {
                vec![cap.usage_min]
            };
            for usage in usages {
                let Some(raw) = ffi::get_usage_value(preparsed, cap.usage_page, usage, &mut owned)?
                else {
                    continue;
                };
                if cap.usage_page == USAGE_PAGE_GENERIC_DESKTOP && usage == 0x39 {
                    let idle = cap.has_null != 0
                        && ((raw as i32) < cap.logical_min || (raw as i32) > cap.logical_max);
                    let name_idx = same_name_count.entry("Hat".to_string()).or_insert(0);
                    let name = if *name_idx == 0 {
                        "Hat".to_string()
                    } else {
                        format!("Hat#{name_idx}")
                    };
                    *name_idx += 1;
                    hats.push(HatValue {
                        usage_page: cap.usage_page,
                        usage,
                        name,
                        raw,
                        logical_min: cap.logical_min,
                        logical_max: cap.logical_max,
                        idle,
                        direction: hat_direction(raw, cap.logical_min, cap.logical_max, idle),
                    });
                    continue;
                }
                let centered = axis_is_centered(cap.usage_page, usage);
                let base = axis_name(cap.usage_page, usage, 0);
                let idx = same_name_count.entry(base.clone()).or_insert(0);
                let name = axis_name(cap.usage_page, usage, *idx);
                *idx += 1;
                let normalized = normalize(raw, cap.logical_min, cap.logical_max, centered);
                axes.push(AxisValue {
                    usage_page: cap.usage_page,
                    usage,
                    name,
                    raw,
                    logical_min: cap.logical_min,
                    logical_max: cap.logical_max,
                    normalized,
                    processed: normalized,
                    centered,
                });
            }
        }

        Ok(X52State {
            report_number,
            elapsed_ms,
            raw: report.to_vec(),
            axes,
            hats,
            buttons_pressed: buttons,
            button_names: Vec::new(),
            mode: ModeSwitch::Unknown,
        })
    }
}
