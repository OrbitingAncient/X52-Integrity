//! Reversible X52 driver switch (needs admin after a confirmed GUI action).
//!
//! App copy / shortcuts live in [`crate::install`]. This module only binds the
//! X52 to inbox `input.inf` or restores the exported Logitech package.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, BOOL, WAIT_OBJECT_0};
use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

use crate::diagnose;
use crate::error::Error;
use crate::hid::to_wide;
use crate::paths;

pub use crate::install::{install_app, open_folder, uninstall_app};
pub use crate::paths::{
    app_installed, desktop_shortcut, install_dir, log_path, running_from_install_dir,
    start_menu_shortcut,
};

/// Hardware ID of the X52 USB function.
pub const USB_HWID: &str = r"USB\VID_06A3&PID_075C";
/// Hardware ID of the X52 HID collection.
pub const HID_HWID: &str = r"HID\VID_06A3&PID_075C";
/// Windows inbox HID INF.
pub const INBOX_INF: &str = r"C:\Windows\INF\input.inf";

const INSTALLFLAG_FORCE: u32 = 0x1;
const INSTALLFLAG_NONINTERACTIVE: u32 = 0x4;

#[link(name = "newdev")]
extern "system" {
    fn UpdateDriverForPlugAndPlayDevicesW(
        hwnd: windows_sys::Win32::Foundation::HWND,
        hardware_id: windows_sys::core::PCWSTR,
        full_inf_path: windows_sys::core::PCWSTR,
        install_flags: u32,
        reboot: *mut BOOL,
    ) -> BOOL;
}

/// Locate the exported Logitech `sai075c.inf` backup.
pub fn logitech_inf() -> Option<PathBuf> {
    let mut starts = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            starts.push(dir.to_path_buf());
        }
    }
    starts.push(paths::install_dir());
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        starts.push(PathBuf::from(manifest));
    }
    if let Ok(cwd) = std::env::current_dir() {
        starts.push(cwd);
    }
    for start in starts {
        let mut dir = start;
        for _ in 0..8 {
            let nested = dir.join(r"backups\sai075c\sai075c.inf");
            if nested.is_file() {
                return Some(nested);
            }
            let tools = dir.join(r"tools\backups\sai075c\sai075c.inf");
            if tools.is_file() {
                return Some(tools);
            }
            if !dir.pop() {
                break;
            }
        }
    }
    None
}

/// Switch the X52 to Windows inbox `input.inf` / HidUsb.
pub fn apply_inbox_hid() -> Result<String, Error> {
    if !diagnose::is_elevated() {
        return Err(Error::NeedAdmin("This action"));
    }
    if !Path::new(INBOX_INF).is_file() {
        return Err(Error::Io(format!("missing {INBOX_INF}")));
    }
    let mut reboot = false;
    reboot |= update_driver(USB_HWID, Path::new(INBOX_INF))?;
    reboot |= update_driver(HID_HWID, Path::new(INBOX_INF))?;
    let _ = restart_x52_usb();
    let extra = if reboot { " Restart required." } else { "" };
    Ok(format!("Bound X52 to Windows HID.{extra}"))
}

/// Restore Logitech `sai075c.inf` from the backup copy.
pub fn restore_logitech() -> Result<String, Error> {
    if !diagnose::is_elevated() {
        return Err(Error::NeedAdmin("This action"));
    }
    let inf = logitech_inf().ok_or_else(|| Error::Io("Logitech driver backup not found".into()))?;
    let reboot = update_driver(USB_HWID, &inf)?;
    let _ = restart_x52_usb();
    let extra = if reboot { " Restart required." } else { "" };
    Ok(format!("Restored Logitech driver.{extra}"))
}

fn update_driver(hwid: &str, inf: &Path) -> Result<bool, Error> {
    let hwid_w = to_wide(hwid);
    let inf_w = to_wide(&inf.display().to_string());
    let mut reboot: BOOL = 0;
    let flags = INSTALLFLAG_FORCE | INSTALLFLAG_NONINTERACTIVE;
    let ok = unsafe {
        UpdateDriverForPlugAndPlayDevicesW(
            std::ptr::null_mut(),
            hwid_w.as_ptr(),
            inf_w.as_ptr(),
            flags,
            &mut reboot,
        )
    };
    if ok == 0 {
        return Err(Error::win32("UpdateDriverForPlugAndPlayDevicesW"));
    }
    Ok(reboot != 0)
}

fn restart_x52_usb() -> Result<(), Error> {
    let out = Command::new("pnputil")
        .args(["/enum-devices", "/connected", "/deviceid", USB_HWID])
        .output()
        .map_err(|e| Error::Io(format!("pnputil: {e}")))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut ids = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Instance ID:") {
            let id = rest.trim();
            if !id.is_empty() {
                ids.push(id.to_string());
            }
        }
    }
    for id in &ids {
        let _ = Command::new("pnputil")
            .args(["/restart-device", id])
            .status();
    }
    if ids.is_empty() {
        let _ = Command::new("pnputil").args(["/scan-devices"]).status();
    }
    Ok(())
}

/// Relaunch this exe elevated with `args`, wait, return the child's exit code.
pub fn relaunch_elevated(args: &str) -> Result<u32, Error> {
    let exe = std::env::current_exe().map_err(|e| Error::Io(e.to_string()))?;
    let exe_w = to_wide(&exe.display().to_string());
    let verb = to_wide("runas");
    let params = to_wide(args);
    let dir = exe
        .parent()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let dir_w = to_wide(&dir);

    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS;
    info.lpVerb = verb.as_ptr();
    info.lpFile = exe_w.as_ptr();
    info.lpParameters = params.as_ptr();
    info.lpDirectory = dir_w.as_ptr();
    info.nShow = SW_HIDE;
    let ok = unsafe { ShellExecuteExW(&mut info) };
    if ok == 0 {
        let code = unsafe { GetLastError() };
        if code == 1223 {
            return Err(Error::Io("UAC cancelled".into()));
        }
        return Err(Error::Win32 {
            api: "ShellExecuteExW",
            code,
        });
    }
    if info.hProcess.is_null() {
        return Ok(0);
    }
    unsafe {
        let wait = WaitForSingleObject(info.hProcess, 120_000);
        let mut code = 0u32;
        let _ = GetExitCodeProcess(info.hProcess, &mut code);
        let _ = CloseHandle(info.hProcess);
        if wait != WAIT_OBJECT_0 {
            return Err(Error::Io(
                "timed out waiting for the elevated helper (120s)".into(),
            ));
        }
        Ok(code)
    }
}

/// Result file written by an elevated helper.
pub fn driver_result_path() -> PathBuf {
    std::env::temp_dir().join("x52-integrity-driver-result.txt")
}

/// Run a driver action in-process (must already be elevated) and write the result file.
pub fn run_elevated_driver_action(action: &str) -> i32 {
    let msg = match action {
        "--driver-inbox" => apply_inbox_hid(),
        "--driver-restore" => restore_logitech(),
        other => Err(Error::Usage(format!("unknown driver action {other}"))),
    };
    let (code, text) = match msg {
        Ok(s) => (0, format!("OK: {s}")),
        Err(e) => (1, format!("ERROR: {e}")),
    };
    let _ = fs::write(driver_result_path(), &text);
    code
}

/// Driver action from the GUI. Elevates only after the caller has confirmed.
pub fn run_driver_action_from_gui(inbox: bool) -> Result<String, Error> {
    let flag = if inbox {
        "--driver-inbox"
    } else {
        "--driver-restore"
    };
    if diagnose::is_elevated() {
        return if inbox {
            apply_inbox_hid()
        } else {
            restore_logitech()
        };
    }
    let _ = fs::remove_file(driver_result_path());
    let code = relaunch_elevated(flag)?;
    let text = fs::read_to_string(driver_result_path())
        .unwrap_or_default()
        .trim()
        .to_string();
    if code != 0 {
        if text.is_empty() {
            Err(Error::Io(format!("elevated helper exited {code}")))
        } else {
            Err(Error::Io(text))
        }
    } else if text.is_empty() {
        Ok("Driver action finished.".into())
    } else {
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_dir_is_per_user() {
        let d = install_dir();
        let name = d.file_name().and_then(|s| s.to_str()).unwrap_or("");
        assert!(
            name.eq_ignore_ascii_case("X52Integrity")
                || name.eq_ignore_ascii_case("X52Firmfix")
                || name == "x52-integrity",
            "{d:?}"
        );
    }

    #[test]
    fn logitech_backup_present_in_repo() {
        let inf = logitech_inf().expect("sai075c.inf backup should exist in tools/backups");
        assert!(inf.ends_with("sai075c.inf"));
        assert!(inf.is_file());
    }

    #[test]
    fn hwid_constants() {
        assert!(USB_HWID.contains("06A3"));
        assert!(HID_HWID.contains("075C"));
        assert!(Path::new(INBOX_INF).is_absolute());
    }
}
