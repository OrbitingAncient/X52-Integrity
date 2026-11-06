//! Windowed X52 Integrity tool. Open and close it like a normal app.
//!
//! Driver install/restore (Windows) is a confirmed button that then shows UAC.
//! The window itself never auto-elevates.

#![cfg_attr(windows, windows_subsystem = "windows")]
#![allow(
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::map_unwrap_or,
    clippy::redundant_closure_for_method_calls,
    clippy::unreadable_literal
)]

mod gui;

use std::io::Write;
use std::path::PathBuf;

use x52_integrity::install;
use x52_integrity::logbuf::Logger;
use x52_integrity::paths;

#[cfg(windows)]
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
#[cfg(windows)]
use windows_sys::Win32::Storage::FileSystem::WriteFile;
#[cfg(windows)]
use windows_sys::Win32::System::Console::{
    AttachConsole, GetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE,
};

fn attach_parent_console() {
    #[cfg(windows)]
    unsafe {
        AttachConsole(0xFFFFFFFF);
    }
}

fn cout(text: &str) {
    #[cfg(windows)]
    unsafe {
        let handles = [
            GetStdHandle(STD_OUTPUT_HANDLE),
            GetStdHandle(STD_ERROR_HANDLE),
        ];
        for h in handles {
            if h.is_null() || h == INVALID_HANDLE_VALUE {
                continue;
            }
            let bytes = text.as_bytes();
            let mut written = 0u32;
            if WriteFile(
                h,
                bytes.as_ptr(),
                bytes.len() as u32,
                &mut written,
                std::ptr::null_mut(),
            ) != 0
            {
                return;
            }
        }
    }
    #[cfg(not(windows))]
    {
        print!("{text}");
        let _ = std::io::stdout().flush();
    }
}

fn coutln(text: &str) {
    cout(text);
    cout("\r\n");
    let _ = std::io::stdout().flush();
}

fn help_text() -> String {
    format!(
        "{}\n\n\
Usage: x52-integrity [option]\n\
\n\
  (no args)            Open the manager\n\
  --self-test          Print device status and exit\n\
  --install            Copy into the per-user folder and create shortcuts\n\
  --uninstall          Remove shortcuts and the local install folder\n\
  --silent             With --install / --uninstall, no window (exit 0/1)\n\
  --prefix DIR         Install/uninstall this folder (sets X52_INTEGRITY_HOME)\n\
  --help               This text\n\
{}",
        x52_integrity::banner(),
        extra_help()
    )
}

fn extra_help() -> &'static str {
    #[cfg(windows)]
    {
        "\n  --driver-inbox       Bind X52 to Windows HID (administrator)\n  --driver-restore     Restore Logitech driver (administrator)\n"
    }
    #[cfg(not(windows))]
    {
        ""
    }
}

fn write_panic_file(info: &str) {
    let path = paths::log_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let panic_path = path.with_file_name("panic.log");
    let _ = std::fs::write(panic_path, info);
}

fn apply_prefix(prefix: Option<PathBuf>) {
    if let Some(p) = prefix {
        std::env::set_var("X52_INTEGRITY_HOME", p);
    }
}

fn self_test() -> i32 {
    let log = Logger::new(Some(paths::log_path()));
    log.info("Device test started");
    coutln(&x52_integrity::banner());

    match x52_integrity::diagnose::collect() {
        Ok(r) => {
            let text = x52_integrity::diagnose::render(&r);
            cout(&text);
            if !text.ends_with('\n') {
                cout("\r\n");
            }
            for line in text.lines() {
                log.info(line);
            }
        }
        Err(e) => {
            coutln(&format!("diagnose: {e}"));
            log.error(format!("diagnose: {e}"));
            return 1;
        }
    }

    match x52_integrity::live::list_x52() {
        Ok(devs) if devs.is_empty() => {
            coutln("No joystick report from the OS");
            log.warn("No joystick report from the OS");
        }
        Ok(devs) => {
            for d in &devs {
                match x52_integrity::live::read_state(d, 0, 0) {
                    Ok(st) => {
                        let axes = st
                            .axes
                            .iter()
                            .map(|a| format!("{}={}", a.name, a.raw))
                            .collect::<Vec<_>>()
                            .join("  ");
                        let hat = st.hats.first().map(|h| h.direction.as_str()).unwrap_or("—");
                        let buttons = if st.buttons_pressed.is_empty() {
                            "none".into()
                        } else {
                            st.buttons_pressed
                                .iter()
                                .map(|b| b.to_string())
                                .collect::<Vec<_>>()
                                .join(",")
                        };
                        let line = format!(
                            "Input  {:04X}:{:04X}  {axes}  hat={hat}  buttons={buttons}",
                            d.vid, d.pid
                        );
                        coutln(&line);
                        log.info(&line);
                    }
                    Err(e) => {
                        coutln(&format!("input read: {e}"));
                        log.error(format!("input read: {e}"));
                    }
                }
            }
        }
        Err(e) => {
            coutln(&format!("input list: {e}"));
            log.error(format!("input list: {e}"));
        }
    }

    #[cfg(windows)]
    match x52_integrity::setup::logitech_inf() {
        Some(p) => coutln(&format!("Backup  {}", p.display())),
        None => coutln("Backup  not found"),
    }
    coutln(&format!("Install {}", paths::install_dir().display()));
    coutln("Device test complete");
    log.info("Device test complete");
    0
}

fn run_install(silent: bool) -> i32 {
    let log = Logger::new(Some(paths::install_log_path()));
    match install::install_app_detailed(Some(&log)) {
        Ok(r) => {
            coutln(&r.message);
            for w in &r.warnings {
                coutln(&format!("warning: {w}"));
            }
            if !silent {
                coutln(&format!("log {}", paths::install_log_path().display()));
            }
            // Shortcuts are warnings; files copied means success.
            0
        }
        Err(e) => {
            coutln(&format!("install failed: {e}"));
            log.error(e.to_string());
            1
        }
    }
}

fn run_uninstall(silent: bool) -> i32 {
    let log = Logger::new(None);
    match install::uninstall_app_detailed(Some(&log)) {
        Ok(msg) => {
            coutln(&msg);
            let _ = silent;
            0
        }
        Err(e) => {
            coutln(&format!("uninstall failed: {e}"));
            log.error(e.to_string());
            1
        }
    }
}

struct Cli {
    silent: bool,
    prefix: Option<PathBuf>,
    action: Action,
}

enum Action {
    Gui,
    Help,
    SelfTest,
    Install,
    Uninstall,
    #[cfg(windows)]
    DriverInbox,
    #[cfg(windows)]
    DriverRestore,
}

fn parse_cli(args: &[String]) -> Result<Cli, String> {
    let mut silent = false;
    let mut prefix = None;
    let mut action = Action::Gui;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => action = Action::Help,
            "--self-test" => action = Action::SelfTest,
            "--install" => action = Action::Install,
            "--uninstall" => action = Action::Uninstall,
            "--silent" | "/S" | "/quiet" | "-q" => silent = true,
            "--prefix" => {
                i += 1;
                let p = args
                    .get(i)
                    .ok_or_else(|| "--prefix needs a directory".to_string())?;
                prefix = Some(PathBuf::from(p));
            }
            #[cfg(windows)]
            "--driver-inbox" => action = Action::DriverInbox,
            #[cfg(windows)]
            "--driver-restore" => action = Action::DriverRestore,
            other => return Err(format!("unknown argument '{other}'. See --help.")),
        }
        i += 1;
    }
    Ok(Cli {
        silent,
        prefix,
        action,
    })
}

fn main() {
    attach_parent_console();
    std::panic::set_hook(Box::new(|info| {
        write_panic_file(&info.to_string());
        eprintln!("{info}");
    }));

    let raw: Vec<String> = std::env::args().skip(1).collect();
    let cli = match parse_cli(&raw) {
        Ok(c) => c,
        Err(e) => {
            coutln(&e);
            std::process::exit(2);
        }
    };
    apply_prefix(cli.prefix);

    match cli.action {
        Action::Help => coutln(&help_text()),
        Action::SelfTest => std::process::exit(self_test()),
        Action::Install => std::process::exit(run_install(cli.silent)),
        Action::Uninstall => std::process::exit(run_uninstall(cli.silent)),
        #[cfg(windows)]
        Action::DriverInbox => {
            std::process::exit(x52_integrity::setup::run_elevated_driver_action(
                "--driver-inbox",
            ));
        }
        #[cfg(windows)]
        Action::DriverRestore => {
            std::process::exit(x52_integrity::setup::run_elevated_driver_action(
                "--driver-restore",
            ));
        }
        Action::Gui => {
            if let Err(e) = gui::run() {
                let msg = format!("GUI error: {e}");
                write_panic_file(&msg);
                coutln(&msg);
                std::process::exit(1);
            }
        }
    }
}
