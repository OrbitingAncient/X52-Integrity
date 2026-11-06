//! HID device discovery. Opens handles only long enough to read identity.

use crate::error::Error;
use crate::hid::ffi::{
    self, create_file_read, enum_interface, get_attributes, interface_detail, manufacturer_string,
    product_string, registry_property, serial_string, DeviceInfoSet,
};
use crate::hid::info::HidDeviceInfo;
use crate::identity::DeviceKind;

/// Every present HID interface we can open far enough to read VID/PID.
pub fn list_all_hid() -> Result<Vec<HidDeviceInfo>, Error> {
    let set = DeviceInfoSet::hid_present()?;
    let mut out = Vec::new();
    for index in 0.. {
        let Some(iface) = enum_interface(&set, index)? else {
            break;
        };
        let detail = match interface_detail(&set, &iface) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let mut opened_with_write_access = false;
        let handle = match create_file_read(&detail.path_wide, false) {
            Ok(h) => h,
            Err(_) => match create_file_read(&detail.path_wide, true) {
                Ok(h) => {
                    opened_with_write_access = true;
                    h
                }
                Err(_) => continue,
            },
        };
        let attrs = match get_attributes(handle.as_raw()) {
            Ok(a) => a,
            Err(_) => continue,
        };
        let kind = DeviceKind::from_vid_pid(attrs.VendorID, attrs.ProductID);
        let service = registry_property(&set, &detail.devinfo, ffi::PROP_SERVICE)
            .into_iter()
            .next()
            .unwrap_or_default();
        let description = registry_property(&set, &detail.devinfo, ffi::PROP_DESC)
            .into_iter()
            .next()
            .unwrap_or_default();
        out.push(HidDeviceInfo {
            path: detail.path,
            vid: attrs.VendorID,
            pid: attrs.ProductID,
            version: attrs.VersionNumber,
            manufacturer: manufacturer_string(handle.as_raw()),
            product: product_string(handle.as_raw()),
            serial: serial_string(handle.as_raw()),
            kind,
            description,
            hardware_ids: registry_property(&set, &detail.devinfo, ffi::PROP_HWID),
            service,
            upper_filters: registry_property(&set, &detail.devinfo, ffi::PROP_UPPER),
            lower_filters: registry_property(&set, &detail.devinfo, ffi::PROP_LOWER),
            opened_with_write_access,
        });
        drop(handle);
    }
    Ok(out)
}

/// X52 / X52 Pro devices only.
pub fn list_x52() -> Result<Vec<HidDeviceInfo>, Error> {
    Ok(list_all_hid()?
        .into_iter()
        .filter(|d| d.kind.is_x52())
        .collect())
}

/// Pick one X52 by index (0 = first).
pub fn pick_x52(index: usize) -> Result<HidDeviceInfo, Error> {
    let list = list_x52()?;
    if list.is_empty() {
        return Err(Error::DeviceNotFound);
    }
    let count = list.len();
    list.into_iter()
        .nth(index)
        .ok_or(Error::DeviceIndexOutOfRange { index, count })
}
