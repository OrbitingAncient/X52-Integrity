//! Isolated unsafe FFI to inbox Windows HID and SetupAPI.
//!
//! No vendor control transfers. `HidD_SetFeature` is not imported unless
//! `write_path` is compiled, and even then it is unused by the monitor.

#![allow(non_snake_case, dead_code, missing_docs)]

use std::ffi::c_void;

use windows_sys::core::GUID;
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
    SetupDiGetDeviceInterfaceDetailW, SetupDiGetDeviceRegistryPropertyW, DIGCF_DEVICEINTERFACE,
    DIGCF_PRESENT, HDEVINFO, SPDRP_DEVICEDESC, SPDRP_HARDWAREID, SPDRP_LOWERFILTERS, SPDRP_SERVICE,
    SPDRP_UPPERFILTERS, SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
    SP_DEVINFO_DATA,
};
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, BOOL, ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS, HANDLE,
    INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT, WIN32_ERROR,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::Threading::{CreateEventW, ResetEvent, WaitForSingleObject};
use windows_sys::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};

pub const GENERIC_READ: u32 = 0x8000_0000;
pub const GENERIC_WRITE: u32 = 0x4000_0000;
pub const FILE_ATTRIBUTE_NORMAL: u32 = 0x0000_0080;

pub const HIDP_STATUS_SUCCESS: i32 = 0x0011_0000;
pub const HIDP_INPUT: i32 = 0;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct HidAttributes {
    pub Size: u32,
    pub VendorID: u16,
    pub ProductID: u16,
    pub VersionNumber: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct HidpCaps {
    pub usage: u16,
    pub usage_page: u16,
    pub input_report_byte_length: u16,
    pub output_report_byte_length: u16,
    pub feature_report_byte_length: u16,
    pub reserved: [u16; 17],
    pub number_link_collection_nodes: u16,
    pub number_input_button_caps: u16,
    pub number_input_value_caps: u16,
    pub number_input_data_indices: u16,
    pub number_output_button_caps: u16,
    pub number_output_value_caps: u16,
    pub number_output_data_indices: u16,
    pub number_feature_button_caps: u16,
    pub number_feature_value_caps: u16,
    pub number_feature_data_indices: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct HidpButtonCaps {
    pub usage_page: u16,
    pub report_id: u8,
    pub is_alias: u8,
    pub bit_field: u16,
    pub link_collection: u16,
    pub link_usage: u16,
    pub link_usage_page: u16,
    pub is_range: u8,
    pub is_string_range: u8,
    pub is_designator_range: u8,
    pub is_absolute: u8,
    pub reserved: [u32; 10],
    pub usage_min: u16,
    pub usage_max: u16,
    pub string_min: u16,
    pub string_max: u16,
    pub designator_min: u16,
    pub designator_max: u16,
    pub data_index_min: u16,
    pub data_index_max: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct HidpValueCaps {
    pub usage_page: u16,
    pub report_id: u8,
    pub is_alias: u8,
    pub bit_field: u16,
    pub link_collection: u16,
    pub link_usage: u16,
    pub link_usage_page: u16,
    pub is_range: u8,
    pub is_string_range: u8,
    pub is_designator_range: u8,
    pub is_absolute: u8,
    pub has_null: u8,
    pub reserved: u8,
    pub bit_size: u16,
    pub report_count: u16,
    pub reserved2: [u16; 5],
    pub units_exp: u32,
    pub units: u32,
    pub logical_min: i32,
    pub logical_max: i32,
    pub physical_min: i32,
    pub physical_max: i32,
    pub usage_min: u16,
    pub usage_max: u16,
    pub string_min: u16,
    pub string_max: u16,
    pub designator_min: u16,
    pub designator_max: u16,
    pub data_index_min: u16,
    pub data_index_max: u16,
}

#[link(name = "hid")]
extern "system" {
    fn HidD_GetHidGuid(guid: *mut GUID);
    fn HidD_GetAttributes(hid: HANDLE, attributes: *mut HidAttributes) -> BOOL;
    fn HidD_GetPreparsedData(hid: HANDLE, preparsed: *mut *mut c_void) -> BOOL;
    fn HidD_FreePreparsedData(preparsed: *mut c_void) -> BOOL;
    fn HidD_GetProductString(hid: HANDLE, buffer: *mut u16, buffer_length: u32) -> BOOL;
    fn HidD_GetManufacturerString(hid: HANDLE, buffer: *mut u16, buffer_length: u32) -> BOOL;
    fn HidD_GetSerialNumberString(hid: HANDLE, buffer: *mut u16, buffer_length: u32) -> BOOL;
    fn HidD_SetNumInputBuffers(hid: HANDLE, number_buffers: u32) -> BOOL;
    fn HidD_FlushQueue(hid: HANDLE) -> BOOL;
    fn HidD_GetInputReport(
        hid: HANDLE,
        report_buffer: *mut c_void,
        report_buffer_length: u32,
    ) -> BOOL;
    fn HidP_GetCaps(preparsed: *mut c_void, caps: *mut HidpCaps) -> i32;
    fn HidP_GetButtonCaps(
        report_type: i32,
        button_caps: *mut HidpButtonCaps,
        button_caps_length: *mut u16,
        preparsed: *mut c_void,
    ) -> i32;
    fn HidP_GetValueCaps(
        report_type: i32,
        value_caps: *mut HidpValueCaps,
        value_caps_length: *mut u16,
        preparsed: *mut c_void,
    ) -> i32;
    fn HidP_GetUsages(
        report_type: i32,
        usage_page: u16,
        link_collection: u16,
        usage_list: *mut u16,
        usage_length: *mut u32,
        preparsed: *mut c_void,
        report: *mut i8,
        report_length: u32,
    ) -> i32;
    fn HidP_GetUsageValue(
        report_type: i32,
        usage_page: u16,
        link_collection: u16,
        usage: u16,
        usage_value: *mut u32,
        preparsed: *mut c_void,
        report: *mut i8,
        report_length: u32,
    ) -> i32;
    fn HidP_MaxUsageListLength(report_type: i32, usage_page: u16, preparsed: *mut c_void) -> u32;
}

pub struct OwnedHandle {
    h: HANDLE,
}

impl OwnedHandle {
    pub fn from_raw(h: HANDLE) -> Option<Self> {
        if h.is_null() || h == INVALID_HANDLE_VALUE {
            None
        } else {
            Some(Self { h })
        }
    }

    pub fn as_raw(&self) -> HANDLE {
        self.h
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.h);
        }
    }
}

pub struct Preparsed {
    ptr: *mut c_void,
}

impl Preparsed {
    pub fn get(handle: HANDLE) -> Result<Self, crate::error::Error> {
        let mut ptr = std::ptr::null_mut();
        let ok = unsafe { HidD_GetPreparsedData(handle, &mut ptr) };
        if ok == 0 || ptr.is_null() {
            return Err(crate::error::Error::win32("HidD_GetPreparsedData"));
        }
        Ok(Self { ptr })
    }

    pub fn as_ptr(&self) -> *mut c_void {
        self.ptr
    }
}

impl Drop for Preparsed {
    fn drop(&mut self) {
        unsafe {
            let _ = HidD_FreePreparsedData(self.ptr);
        }
    }
}

pub fn hid_guid() -> GUID {
    let mut guid = GUID {
        data1: 0,
        data2: 0,
        data3: 0,
        data4: [0; 8],
    };
    unsafe { HidD_GetHidGuid(&mut guid) };
    guid
}

pub fn create_file_read(
    path_wide: &[u16],
    write_access: bool,
) -> Result<OwnedHandle, crate::error::Error> {
    let access = if write_access {
        GENERIC_READ | GENERIC_WRITE
    } else {
        GENERIC_READ
    };
    let handle = unsafe {
        CreateFileW(
            path_wide.as_ptr(),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_OVERLAPPED | FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    };
    OwnedHandle::from_raw(handle).ok_or_else(|| crate::error::Error::win32("CreateFileW"))
}

pub fn get_attributes(handle: HANDLE) -> Result<HidAttributes, crate::error::Error> {
    let mut a = HidAttributes {
        Size: std::mem::size_of::<HidAttributes>() as u32,
        VendorID: 0,
        ProductID: 0,
        VersionNumber: 0,
    };
    let ok = unsafe { HidD_GetAttributes(handle, &mut a) };
    if ok == 0 {
        return Err(crate::error::Error::win32("HidD_GetAttributes"));
    }
    Ok(a)
}

pub fn get_hid_string(
    handle: HANDLE,
    which: fn(HANDLE, *mut u16, u32) -> BOOL,
    api: &'static str,
) -> String {
    let mut buf = [0u16; 126];
    let ok = which(handle, buf.as_mut_ptr(), (buf.len() * 2) as u32);
    if ok == 0 {
        let _ = api;
        return String::new();
    }
    wide_to_string(&buf)
}

pub fn product_string(handle: HANDLE) -> String {
    get_hid_string(
        handle,
        |h, b, l| unsafe { HidD_GetProductString(h, b, l) },
        "HidD_GetProductString",
    )
}

pub fn manufacturer_string(handle: HANDLE) -> String {
    get_hid_string(
        handle,
        |h, b, l| unsafe { HidD_GetManufacturerString(h, b, l) },
        "HidD_GetManufacturerString",
    )
}

pub fn serial_string(handle: HANDLE) -> String {
    get_hid_string(
        handle,
        |h, b, l| unsafe { HidD_GetSerialNumberString(h, b, l) },
        "HidD_GetSerialNumberString",
    )
}

pub fn set_num_input_buffers(handle: HANDLE, count: u32) -> bool {
    unsafe { HidD_SetNumInputBuffers(handle, count) != 0 }
}

pub fn flush_queue(handle: HANDLE) -> bool {
    unsafe { HidD_FlushQueue(handle) != 0 }
}

/// Control-pipe GET_REPORT. Still a read, never a vendor 0x91 write.
pub fn get_input_report(handle: HANDLE, buf: &mut [u8]) -> Result<usize, crate::error::Error> {
    let ok =
        unsafe { HidD_GetInputReport(handle, buf.as_mut_ptr() as *mut c_void, buf.len() as u32) };
    if ok == 0 {
        return Err(crate::error::Error::win32("HidD_GetInputReport"));
    }
    Ok(buf.len())
}

pub fn get_caps(preparsed: &Preparsed) -> Result<HidpCaps, crate::error::Error> {
    let mut caps = unsafe { std::mem::zeroed::<HidpCaps>() };
    let st = unsafe { HidP_GetCaps(preparsed.as_ptr(), &mut caps) };
    if st != HIDP_STATUS_SUCCESS {
        return Err(crate::error::Error::Win32 {
            api: "HidP_GetCaps",
            code: st as u32,
        });
    }
    Ok(caps)
}

pub fn get_button_caps(
    preparsed: &Preparsed,
    count: u16,
) -> Result<Vec<HidpButtonCaps>, crate::error::Error> {
    if count == 0 {
        return Ok(Vec::new());
    }
    let mut caps = vec![unsafe { std::mem::zeroed::<HidpButtonCaps>() }; count as usize];
    let mut len = count;
    let st =
        unsafe { HidP_GetButtonCaps(HIDP_INPUT, caps.as_mut_ptr(), &mut len, preparsed.as_ptr()) };
    if st != HIDP_STATUS_SUCCESS {
        return Err(crate::error::Error::Win32 {
            api: "HidP_GetButtonCaps",
            code: st as u32,
        });
    }
    caps.truncate(len as usize);
    Ok(caps)
}

pub fn get_value_caps(
    preparsed: &Preparsed,
    count: u16,
) -> Result<Vec<HidpValueCaps>, crate::error::Error> {
    if count == 0 {
        return Ok(Vec::new());
    }
    let mut caps = vec![unsafe { std::mem::zeroed::<HidpValueCaps>() }; count as usize];
    let mut len = count;
    let st =
        unsafe { HidP_GetValueCaps(HIDP_INPUT, caps.as_mut_ptr(), &mut len, preparsed.as_ptr()) };
    if st != HIDP_STATUS_SUCCESS {
        return Err(crate::error::Error::Win32 {
            api: "HidP_GetValueCaps",
            code: st as u32,
        });
    }
    caps.truncate(len as usize);
    Ok(caps)
}

pub fn get_usages(
    preparsed: &Preparsed,
    usage_page: u16,
    report: &mut [u8],
) -> Result<Vec<u16>, crate::error::Error> {
    let max = unsafe { HidP_MaxUsageListLength(HIDP_INPUT, usage_page, preparsed.as_ptr()) };
    if max == 0 {
        return Ok(Vec::new());
    }
    let mut list = vec![0u16; max as usize];
    let mut len = max;
    let st = unsafe {
        HidP_GetUsages(
            HIDP_INPUT,
            usage_page,
            0,
            list.as_mut_ptr(),
            &mut len,
            preparsed.as_ptr(),
            report.as_mut_ptr() as *mut i8,
            report.len() as u32,
        )
    };
    if st != HIDP_STATUS_SUCCESS {
        return Ok(Vec::new());
    }
    list.truncate(len as usize);
    Ok(list)
}

pub fn get_usage_value(
    preparsed: &Preparsed,
    usage_page: u16,
    usage: u16,
    report: &mut [u8],
) -> Result<Option<u32>, crate::error::Error> {
    let mut value = 0u32;
    let st = unsafe {
        HidP_GetUsageValue(
            HIDP_INPUT,
            usage_page,
            0,
            usage,
            &mut value,
            preparsed.as_ptr(),
            report.as_mut_ptr() as *mut i8,
            report.len() as u32,
        )
    };
    if st != HIDP_STATUS_SUCCESS {
        return Ok(None);
    }
    Ok(Some(value))
}

pub fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

pub fn to_wide(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

pub struct DeviceInfoSet {
    h: HDEVINFO,
}

impl DeviceInfoSet {
    pub fn hid_present() -> Result<Self, crate::error::Error> {
        let guid = hid_guid();
        let h = unsafe {
            SetupDiGetClassDevsW(
                &guid,
                std::ptr::null(),
                std::ptr::null_mut(),
                DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
            )
        };
        if h == 0 || h == -1 {
            return Err(crate::error::Error::win32("SetupDiGetClassDevsW"));
        }
        Ok(Self { h })
    }

    pub fn raw(&self) -> HDEVINFO {
        self.h
    }
}

impl Drop for DeviceInfoSet {
    fn drop(&mut self) {
        unsafe {
            let _ = SetupDiDestroyDeviceInfoList(self.h);
        }
    }
}

pub struct InterfaceDetail {
    pub path: String,
    pub path_wide: Vec<u16>,
    pub devinfo: SP_DEVINFO_DATA,
}

pub fn enum_interface(
    set: &DeviceInfoSet,
    index: u32,
) -> Result<Option<SP_DEVICE_INTERFACE_DATA>, crate::error::Error> {
    let guid = hid_guid();
    let mut iface: SP_DEVICE_INTERFACE_DATA = unsafe { std::mem::zeroed() };
    iface.cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;
    let ok = unsafe {
        SetupDiEnumDeviceInterfaces(set.raw(), std::ptr::null(), &guid, index, &mut iface)
    };
    if ok == 0 {
        let err = unsafe { GetLastError() };
        if err == ERROR_NO_MORE_ITEMS {
            return Ok(None);
        }
        return Err(crate::error::Error::Win32 {
            api: "SetupDiEnumDeviceInterfaces",
            code: err,
        });
    }
    Ok(Some(iface))
}

pub fn interface_detail(
    set: &DeviceInfoSet,
    iface: &SP_DEVICE_INTERFACE_DATA,
) -> Result<InterfaceDetail, crate::error::Error> {
    let mut needed = 0u32;
    let _ = unsafe {
        SetupDiGetDeviceInterfaceDetailW(
            set.raw(),
            iface,
            std::ptr::null_mut(),
            0,
            &mut needed,
            std::ptr::null_mut(),
        )
    };
    let err = unsafe { GetLastError() };
    if needed == 0 && err != ERROR_INSUFFICIENT_BUFFER {
        return Err(crate::error::Error::Win32 {
            api: "SetupDiGetDeviceInterfaceDetailW(size)",
            code: err,
        });
    }
    let mut buf = vec![0u8; needed as usize];
    let detail = buf.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
    unsafe {
        (*detail).cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
    }
    let mut devinfo: SP_DEVINFO_DATA = unsafe { std::mem::zeroed() };
    devinfo.cbSize = std::mem::size_of::<SP_DEVINFO_DATA>() as u32;
    let ok = unsafe {
        SetupDiGetDeviceInterfaceDetailW(
            set.raw(),
            iface,
            detail,
            needed,
            std::ptr::null_mut(),
            &mut devinfo,
        )
    };
    if ok == 0 {
        return Err(crate::error::Error::win32(
            "SetupDiGetDeviceInterfaceDetailW",
        ));
    }
    let path_ptr = unsafe { (*detail).DevicePath.as_ptr() };
    let path_wide = unsafe { wide_from_ptr(path_ptr) };
    let path = wide_to_string(&path_wide);
    Ok(InterfaceDetail {
        path,
        path_wide,
        devinfo,
    })
}

unsafe fn wide_from_ptr(ptr: *const u16) -> Vec<u16> {
    let mut n = 0usize;
    unsafe {
        while *ptr.add(n) != 0 {
            n += 1;
            if n > 4096 {
                break;
            }
        }
        let mut v = std::slice::from_raw_parts(ptr, n).to_vec();
        v.push(0);
        v
    }
}

pub fn registry_property(set: &DeviceInfoSet, devinfo: &SP_DEVINFO_DATA, prop: u32) -> Vec<String> {
    let mut needed = 0u32;
    let mut dtype = 0u32;
    let _ = unsafe {
        SetupDiGetDeviceRegistryPropertyW(
            set.raw(),
            devinfo,
            prop,
            &mut dtype,
            std::ptr::null_mut(),
            0,
            &mut needed,
        )
    };
    if needed == 0 {
        return Vec::new();
    }
    let mut buf = vec![0u8; needed as usize];
    let ok = unsafe {
        SetupDiGetDeviceRegistryPropertyW(
            set.raw(),
            devinfo,
            prop,
            &mut dtype,
            buf.as_mut_ptr(),
            needed,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Vec::new();
    }
    parse_reg_strings(&buf)
}

fn parse_reg_strings(buf: &[u8]) -> Vec<String> {
    if buf.len() < 2 {
        return Vec::new();
    }
    let n = buf.len() / 2;
    let mut wide = Vec::with_capacity(n);
    for i in 0..n {
        let lo = buf[i * 2] as u16;
        let hi = buf[i * 2 + 1] as u16;
        wide.push(lo | (hi << 8));
    }
    let mut out = Vec::new();
    let mut start = 0usize;
    for i in 0..wide.len() {
        if wide[i] == 0 {
            if i > start {
                out.push(String::from_utf16_lossy(&wide[start..i]));
            }
            start = i + 1;
        }
    }
    out.into_iter().filter(|s| !s.is_empty()).collect()
}

pub fn read_overlapped(
    handle: HANDLE,
    buf: &mut [u8],
    timeout_ms: u32,
) -> Result<Option<usize>, crate::error::Error> {
    let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
    if event.is_null() || event == INVALID_HANDLE_VALUE {
        return Err(crate::error::Error::win32("CreateEventW"));
    }
    struct EventGuard(HANDLE);
    impl Drop for EventGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    let _eg = EventGuard(event);

    let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
    overlapped.hEvent = event;
    let _ = unsafe { ResetEvent(event) };

    // Overlapped ReadFile requires lpNumberOfBytesRead = NULL.
    // Even when ReadFile returns TRUE (completed inline), the byte count
    // comes from GetOverlappedResult — not from a dummy out-param.
    let ok = unsafe {
        windows_sys::Win32::Storage::FileSystem::ReadFile(
            handle,
            buf.as_mut_ptr(),
            buf.len() as u32,
            std::ptr::null_mut(),
            &mut overlapped,
        )
    };
    if ok == 0 {
        let err = unsafe { GetLastError() };
        const ERROR_IO_PENDING: WIN32_ERROR = 997;
        if err != ERROR_IO_PENDING {
            return Err(crate::error::Error::Win32 {
                api: "ReadFile",
                code: err,
            });
        }
        let wr = unsafe { WaitForSingleObject(event, timeout_ms) };
        if wr == WAIT_TIMEOUT {
            let _ = unsafe { CancelIoEx(handle, &overlapped) };
            let mut ignored = 0u32;
            let _ = unsafe { GetOverlappedResult(handle, &overlapped, &mut ignored, 1) };
            return Ok(None);
        }
        if wr != WAIT_OBJECT_0 {
            return Err(crate::error::Error::Win32 {
                api: "WaitForSingleObject",
                code: wr,
            });
        }
    }
    let mut read = 0u32;
    let ok = unsafe { GetOverlappedResult(handle, &overlapped, &mut read, 0) };
    if ok == 0 {
        return Err(crate::error::Error::win32("GetOverlappedResult"));
    }
    Ok(Some(read as usize))
}

pub fn last_error() -> u32 {
    unsafe { GetLastError() }
}

pub const PROP_SERVICE: u32 = SPDRP_SERVICE;
pub const PROP_UPPER: u32 = SPDRP_UPPERFILTERS;
pub const PROP_LOWER: u32 = SPDRP_LOWERFILTERS;
pub const PROP_HWID: u32 = SPDRP_HARDWAREID;
pub const PROP_DESC: u32 = SPDRP_DEVICEDESC;
