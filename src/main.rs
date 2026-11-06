//! `x52-monitor` — read-only X52 HID dashboard.
//!
//! This binary has no code path that sends vendor 0x91 commands, writes a
//! virtual joystick, injects keys, or flashes firmware.

use x52_integrity::error::Error;
use x52_integrity::monitor;

fn main() {
    let args = match monitor::Args::parse(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(Error::Usage(msg)) => {
            let is_help = msg.contains("Usage:");
            eprintln!("{msg}");
            std::process::exit(if is_help { 0 } else { 2 });
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    if let Err(e) = monitor::run(args) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
