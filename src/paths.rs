//! Install, config, and log locations. Per-user, no admin required.

use std::path::PathBuf;

/// Current product folder name.
pub const PRODUCT_DIR: &str = "X52Integrity";
/// Previous product folder (migrated on first new install).
pub const LEGACY_DIR: &str = "X52Firmfix";
/// GUI executable file name.
pub const GUI_EXE: &str = if cfg!(windows) {
    "x52-integrity.exe"
} else {
    "x52-integrity"
};
/// Console monitor file name.
pub const MONITOR_EXE: &str = if cfg!(windows) {
    "x52-monitor.exe"
} else {
    "x52-monitor"
};

/// Per-user data root (`%LOCALAPPDATA%\X52Integrity` or `~/.local/share/x52-integrity`).
///
/// Override with `X52_INTEGRITY_HOME` (portable zip, per-machine Program Files,
/// or a CI prefix).
pub fn data_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("X52_INTEGRITY_HOME") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    #[cfg(windows)]
    {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
        let neu = base.join(PRODUCT_DIR);
        let old = base.join(LEGACY_DIR);
        if !neu.exists() && old.exists() {
            return old;
        }
        neu
    }
    #[cfg(not(windows))]
    {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("/tmp"))
                    .join(".local/share")
            });
        base.join("x52-integrity")
    }
}

/// Alias used by the rest of the crate.
pub fn install_dir() -> PathBuf {
    data_dir()
}

/// Log file path.
pub fn log_path() -> PathBuf {
    data_dir().join("logs").join("x52-integrity.log")
}

/// Installer transaction log.
pub fn install_log_path() -> PathBuf {
    data_dir().join("logs").join("install.log")
}

/// Install manifest written after a successful copy.
pub fn manifest_path() -> PathBuf {
    data_dir().join("install-manifest.json")
}

/// First-run / user config.
pub fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

/// Desktop shortcut (Windows) or `.desktop` (Linux).
pub fn desktop_shortcut() -> PathBuf {
    #[cfg(windows)]
    {
        user_profile().join("Desktop").join("X52 Integrity.lnk")
    }
    #[cfg(not(windows))]
    {
        user_home().join(".local/share/applications/x52-integrity.desktop")
    }
}

/// Start Menu shortcut (Windows) or empty on Linux (desktop file covers it).
pub fn start_menu_shortcut() -> PathBuf {
    #[cfg(windows)]
    {
        appdata().join(r"Microsoft\Windows\Start Menu\Programs\X52 Integrity\X52 Integrity.lnk")
    }
    #[cfg(not(windows))]
    {
        user_home().join(".local/share/applications/x52-integrity.desktop")
    }
}

#[cfg(windows)]
fn user_profile() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Users\Public"))
}

#[cfg(windows)]
fn appdata() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| user_profile().join("AppData").join("Roaming"))
}

#[cfg(not(windows))]
fn user_home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// True when this process is running from the per-user install directory.
pub fn running_from_install_dir() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let Ok(exe) = exe.canonicalize() else {
        return false;
    };
    let Ok(dir) = install_dir().canonicalize() else {
        return false;
    };
    exe.starts_with(&dir)
}

/// True if a GUI binary exists in the per-user install directory.
pub fn app_installed() -> bool {
    install_dir().join(GUI_EXE).is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dir_named_for_product() {
        let d = data_dir();
        let name = d.file_name().and_then(|s| s.to_str()).unwrap_or("");
        assert!(
            name.eq_ignore_ascii_case(PRODUCT_DIR) || name == "x52-integrity" || name == LEGACY_DIR,
            "{d:?}"
        );
    }
}
