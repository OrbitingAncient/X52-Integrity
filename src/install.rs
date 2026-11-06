//! Transactional per-user install / uninstall.
//!
//! Copies binaries and optional driver backups, writes a manifest, creates
//! shortcuts, and can roll back if a required step fails. Shortcut failure
//! does not roll back the file copy (the app still runs).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::logbuf::Logger;
use crate::paths;

/// Record of what an install actually wrote, used for uninstall and rollback.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstallManifest {
    /// ISO-ish unix seconds.
    pub installed_at: u64,
    /// Product version.
    pub version: String,
    /// Install root.
    pub install_dir: String,
    /// Files copied (absolute).
    pub files: Vec<String>,
    /// Shortcut paths created.
    pub shortcuts: Vec<String>,
    /// True when Desktop/Start Menu links were created.
    pub shortcuts_ok: bool,
}

/// Install result for CLI/GUI.
#[derive(Debug, Clone)]
pub struct InstallReport {
    /// Human summary.
    pub message: String,
    /// Manifest after success.
    pub manifest: InstallManifest,
    /// Non-fatal problems (usually shortcuts).
    pub warnings: Vec<String>,
}

/// Copy this process into the per-user data directory.
pub fn install_app() -> Result<String, Error> {
    let report = install_app_detailed(None)?;
    Ok(report.message)
}

/// Install with an optional logger (installer CLI, GUI, PowerShell wrapper).
///
/// File-copy failure rolls back anything already written. Shortcut failure is
/// a warning: the binaries are still usable from the install folder.
pub fn install_app_detailed(log: Option<&Logger>) -> Result<InstallReport, Error> {
    let dest = paths::install_dir();
    let log_dir = dest.join("logs");
    fs::create_dir_all(&log_dir)?;
    let logger;
    let log = match log {
        Some(l) => l,
        None => {
            logger = Logger::new(Some(paths::install_log_path()));
            &logger
        }
    };
    log.info(format!("Install start → {}", dest.display()));

    let mut manifest = InstallManifest {
        installed_at: unix_now(),
        version: crate::VERSION.to_string(),
        install_dir: dest.display().to_string(),
        files: Vec::new(),
        shortcuts: Vec::new(),
        shortcuts_ok: false,
    };

    match install_copy_and_link(&dest, &mut manifest, log) {
        Ok(report) => Ok(report),
        Err(e) => {
            log.error(format!("Install failed: {e}; rolling back"));
            let _ = rollback(&manifest);
            Err(e)
        }
    }
}

fn install_copy_and_link(
    dest: &Path,
    manifest: &mut InstallManifest,
    log: &Logger,
) -> Result<InstallReport, Error> {
    let mut warnings = Vec::new();

    let src_exe = std::env::current_exe()?;
    let dest_gui = dest.join(paths::GUI_EXE);
    copy_file_atomic(&src_exe, &dest_gui, log)?;
    manifest.files.push(dest_gui.display().to_string());

    if let Some(dir) = src_exe.parent() {
        let mon_src = dir.join(paths::MONITOR_EXE);
        if mon_src.is_file() {
            let mon_dst = dest.join(paths::MONITOR_EXE);
            copy_file_atomic(&mon_src, &mon_dst, log)?;
            manifest.files.push(mon_dst.display().to_string());
        }
        // Keep a compatibility copy of the old GUI name so existing shortcuts keep working.
        #[cfg(windows)]
        {
            let legacy = dest.join("x52-firmfix.exe");
            if dest_gui.is_file() {
                let _ = fs::copy(&dest_gui, &legacy);
                if legacy.is_file() {
                    manifest.files.push(legacy.display().to_string());
                }
            }
        }
        copy_backup_tree(
            &find_backup_dir(),
            &dest.join("backups").join("sai075c"),
            manifest,
            log,
        );
        #[cfg(unix)]
        {
            let udev_src = find_udev_rule(dir);
            if let Some(src) = udev_src {
                let udev_dst = dest.join("99-x52-integrity.rules");
                copy_file_atomic(&src, &udev_dst, log)?;
                manifest.files.push(udev_dst.display().to_string());
            }
        }
    }

    match create_shortcut(&paths::desktop_shortcut(), &dest_gui, dest) {
        Ok(()) => {
            manifest
                .shortcuts
                .push(paths::desktop_shortcut().display().to_string());
            log.info("Desktop shortcut created");
        }
        Err(e) => {
            warnings.push(format!("Desktop shortcut: {e}"));
            log.warn(format!("Desktop shortcut: {e}"));
        }
    }
    match create_shortcut(&paths::start_menu_shortcut(), &dest_gui, dest) {
        Ok(()) => {
            manifest
                .shortcuts
                .push(paths::start_menu_shortcut().display().to_string());
            log.info("Start Menu shortcut created");
        }
        Err(e) => {
            warnings.push(format!("Start Menu shortcut: {e}"));
            log.warn(format!("Start Menu shortcut: {e}"));
        }
    }
    manifest.shortcuts_ok = warnings.is_empty();

    write_manifest(&manifest)?;
    log.info("Manifest written");

    let mut parts = vec![format!("Installed to {}", dest.display())];
    if manifest.shortcuts_ok {
        parts.push("shortcuts created".into());
    } else {
        parts.push(format!("app files copied ({})", warnings.join("; ")));
    }
    let message = parts.join(" — ");
    log.info(&message);
    Ok(InstallReport {
        message,
        manifest: manifest.clone(),
        warnings,
    })
}

/// Remove shortcuts and the per-user folder.
pub fn uninstall_app() -> Result<String, Error> {
    uninstall_app_detailed(None)
}

/// Uninstall with logging. If we are running from the install dir, delay delete.
pub fn uninstall_app_detailed(log: Option<&Logger>) -> Result<String, Error> {
    let dest = paths::install_dir();
    let logger;
    let log = match log {
        Some(l) => l,
        None => {
            logger = Logger::new(None);
            &logger
        }
    };
    log.info("Uninstall start");

    let manifest = read_manifest().unwrap_or_default();
    for s in manifest.shortcuts.iter().chain(
        [
            paths::desktop_shortcut().display().to_string(),
            paths::start_menu_shortcut().display().to_string(),
        ]
        .iter(),
    ) {
        let p = PathBuf::from(s);
        let _ = fs::remove_file(&p);
        if let Some(parent) = p.parent() {
            let _ = fs::remove_dir(parent);
        }
        log.info(format!("Removed {s}"));
    }

    if paths::running_from_install_dir() {
        schedule_delete_after_exit(&dest);
        return Ok("Uninstalled. This window will close so the folder can be removed.".into());
    }
    if dest.exists() {
        fs::remove_dir_all(&dest)?;
    }
    Ok(format!("Removed {}", dest.display()))
}

/// Roll back a failed install using the last manifest (best-effort).
pub fn rollback(manifest: &InstallManifest) -> Result<(), Error> {
    for s in &manifest.shortcuts {
        let _ = fs::remove_file(s);
    }
    for f in manifest.files.iter().rev() {
        let _ = fs::remove_file(f);
    }
    Ok(())
}

fn write_manifest(m: &InstallManifest) -> Result<(), Error> {
    let path = paths::manifest_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(m).map_err(|e| Error::Io(e.to_string()))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text)?;
    fs::rename(tmp, path)?;
    Ok(())
}

fn read_manifest() -> Option<InstallManifest> {
    let text = fs::read_to_string(paths::manifest_path()).ok()?;
    serde_json::from_str(&text).ok()
}

fn copy_file_atomic(src: &Path, dst: &Path, log: &Logger) -> Result<(), Error> {
    if src == dst {
        return Ok(());
    }
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = dst.with_extension("copying");
    fs::copy(src, &tmp)?;
    fs::rename(&tmp, dst)?;
    log.info(format!("Copied {}", dst.display()));
    Ok(())
}

fn copy_backup_tree(
    from: &Option<PathBuf>,
    to: &Path,
    manifest: &mut InstallManifest,
    log: &Logger,
) {
    let Some(from) = from else {
        return;
    };
    if !from.is_dir() {
        return;
    }
    let _ = fs::create_dir_all(to);
    if let Ok(rd) = fs::read_dir(from) {
        for e in rd.flatten() {
            if e.path().is_file() {
                let t = to.join(e.file_name());
                if fs::copy(e.path(), &t).is_ok() {
                    manifest.files.push(t.display().to_string());
                }
            }
        }
        log.info(format!("Copied driver backup from {}", from.display()));
    }
}

fn find_backup_dir() -> Option<PathBuf> {
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
            let nested = dir.join("backups").join("sai075c");
            if nested.is_dir() {
                return Some(nested);
            }
            let tools = dir.join("tools").join("backups").join("sai075c");
            if tools.is_dir() {
                return Some(tools);
            }
            if !dir.pop() {
                break;
            }
        }
    }
    None
}

#[cfg(unix)]
fn find_udev_rule(exe_dir: &Path) -> Option<PathBuf> {
    let mut dir = exe_dir.to_path_buf();
    for _ in 0..8 {
        let p = dir.join("packaging/linux/99-x52-integrity.rules");
        if p.is_file() {
            return Some(p);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn create_shortcut(lnk: &Path, target: &Path, workdir: &Path) -> Result<(), Error> {
    if let Some(parent) = lnk.parent() {
        fs::create_dir_all(parent)?;
    }
    #[cfg(windows)]
    {
        if shortcut_via_powershell(lnk, target, workdir).is_ok() {
            return Ok(());
        }
        shortcut_via_wscript(lnk, target, workdir)
    }
    #[cfg(unix)]
    {
        write_desktop_file(lnk, target, workdir)
    }
}

#[cfg(windows)]
fn shortcut_via_powershell(lnk: &Path, target: &Path, workdir: &Path) -> Result<(), Error> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let script = format!(
        "$s = (New-Object -ComObject WScript.Shell).CreateShortcut('{}'); $s.TargetPath = '{}'; $s.WorkingDirectory = '{}'; $s.Description = 'X52 Integrity'; $s.IconLocation = '{},0'; $s.Save()",
        escape_ps(lnk),
        escape_ps(target),
        escape_ps(workdir),
        escape_ps(target),
    );
    let exe = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe";
    if !Path::new(exe).is_file() {
        return Err(Error::Io("Windows PowerShell missing".into()));
    }
    let st = std::process::Command::new(exe)
        .args(["-NoProfile", "-STA", "-NonInteractive", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .status()?;
    if st.success() && lnk.is_file() {
        Ok(())
    } else {
        Err(Error::Io("PowerShell shortcut failed".into()))
    }
}

#[cfg(windows)]
fn shortcut_via_wscript(lnk: &Path, target: &Path, workdir: &Path) -> Result<(), Error> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let vbs = format!(
        "Set s = CreateObject(\"WScript.Shell\")\r\nSet sc = s.CreateShortcut(\"{}\")\r\nsc.TargetPath = \"{}\"\r\nsc.WorkingDirectory = \"{}\"\r\nsc.WindowStyle = 1\r\nsc.Description = \"X52 Integrity\"\r\nsc.IconLocation = \"{},0\"\r\nsc.Save\r\n",
        escape_vbs(lnk),
        escape_vbs(target),
        escape_vbs(workdir),
        escape_vbs(target),
    );
    let tmp =
        std::env::temp_dir().join(format!("x52-integrity-shortcut-{}.vbs", std::process::id()));
    fs::write(&tmp, vbs)?;
    let result = std::process::Command::new(r"C:\Windows\System32\cscript.exe")
        .args(["//Nologo", &tmp.display().to_string()])
        .creation_flags(CREATE_NO_WINDOW)
        .status();
    let _ = fs::remove_file(&tmp);
    let st = result?;
    if st.success() && lnk.is_file() {
        Ok(())
    } else {
        Err(Error::Io("could not create shortcut".into()))
    }
}

#[cfg(windows)]
fn escape_ps(p: &Path) -> String {
    p.display().to_string().replace('\'', "''")
}

#[cfg(windows)]
fn escape_vbs(p: &Path) -> String {
    p.display().to_string().replace('"', "\"\"")
}

#[cfg(unix)]
fn write_desktop_file(lnk: &Path, target: &Path, workdir: &Path) -> Result<(), Error> {
    let body = format!(
        "[Desktop Entry]\nType=Application\nName=X52 Integrity\nComment=Saitek/Logitech X52 companion\nExec=\"{}\"\nPath={}\nIcon=input-gaming\nTerminal=false\nCategories=Utility;Game;\n",
        target.display(),
        workdir.display()
    );
    fs::write(lnk, body)?;
    Ok(())
}

#[cfg(windows)]
fn schedule_delete_after_exit(dest: &Path) {
    let cmd = format!(
        "ping 127.0.0.1 -n 3 > nul & rmdir /S /Q \"{}\"",
        dest.display()
    );
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "/MIN", "cmd", "/C", &cmd])
        .spawn();
}

#[cfg(unix)]
fn schedule_delete_after_exit(dest: &Path) {
    let cmd = format!("sleep 2; rm -rf '{}'", dest.display());
    let _ = std::process::Command::new("sh").args(["-c", &cmd]).spawn();
}

/// Open a folder in the platform file manager.
pub fn open_folder(path: &Path) -> Result<(), Error> {
    let dir = if path.is_file() {
        path.parent().unwrap_or(path).to_path_buf()
    } else {
        path.to_path_buf()
    };
    fs::create_dir_all(&dir)?;
    #[cfg(windows)]
    {
        std::process::Command::new("explorer").arg(&dir).spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(&dir).spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(&dir).spawn()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_roundtrip_shape() {
        let m = InstallManifest {
            installed_at: 1,
            version: "0.2.0".into(),
            install_dir: "/tmp".into(),
            files: vec!["a".into()],
            shortcuts: vec![],
            shortcuts_ok: false,
        };
        let s = serde_json::to_string(&m).expect("json");
        let q: InstallManifest = serde_json::from_str(&s).expect("parse");
        assert_eq!(q.files.len(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn shortcut_creates_lnk() {
        let dir = std::env::temp_dir().join("x52-integrity-shortcut-test");
        let _ = fs::create_dir_all(&dir);
        let lnk = dir.join("X52 Integrity.lnk");
        let _ = fs::remove_file(&lnk);
        create_shortcut(&lnk, Path::new(r"C:\Windows\System32\notepad.exe"), &dir)
            .expect("shortcut");
        assert!(lnk.is_file());
        let _ = fs::remove_file(&lnk);
    }
}
