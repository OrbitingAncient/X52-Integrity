//! X52 Integrity — user-space X52 HOTAS companion (Windows 10/11 and Linux).
//!
//! Monitor-first: enumerate, parse, curve, display. Hardware writes require
//! the `write_path` feature **and** an armed [`safety::SafetyGate`].
//!
//! ```text
//! Physical X52 USB
//!        │
//!        ▼
//! Windows: hidusb.sys / hidclass.sys     (HVCI / Memory Integrity OK)
//! Linux:   hid-generic + evdev + udev
//!        │
//!        ▼
//! x52-integrity  (normal user process)
//!   Enumerator → live input (winmm / evdev) → curves → UI
//!   SafetyGate deny-by-default
//! ```

#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::module_name_repetitions,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_lossless,
    clippy::cast_ptr_alignment,
    clippy::doc_markdown,
    clippy::format_push_string,
    clippy::ptr_as_ptr,
    clippy::borrow_as_ptr,
    clippy::ref_as_ptr,
    clippy::struct_excessive_bools,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::return_self_not_must_use,
    clippy::float_cmp,
    clippy::assertions_on_constants,
    clippy::bool_to_int_with_if,
    clippy::items_after_statements,
    clippy::needless_pass_by_value,
    clippy::needless_borrow,
    clippy::ref_option,
    clippy::single_match,
    clippy::single_match_else,
    clippy::manual_let_else,
    clippy::map_unwrap_or,
    clippy::field_reassign_with_default,
    clippy::if_same_then_else,
    clippy::unchecked_time_subtraction,
    clippy::derivable_impls
)]

pub mod config;
#[cfg(windows)]
pub mod diagnose;
#[cfg(not(windows))]
#[path = "diagnose_linux.rs"]
pub mod diagnose;
pub mod error;
pub mod guard;
pub mod hid;
pub mod identity;
pub mod install;
#[cfg(unix)]
pub mod linux_input;
pub mod live;
pub mod logbuf;
pub mod map;
pub mod monitor;
pub mod output;
pub mod paths;
pub mod pipeline;
pub mod profile;
pub mod safety;
#[cfg(windows)]
pub mod setup;
pub mod state;
pub mod vendor;
#[cfg(windows)]
pub mod winmm;

pub use error::Error;
pub use identity::{DeviceKind, SAITEK_VID, X52_PIDS};
pub use safety::{RunMode, SafetyError, SafetyGate};
pub use state::X52State;

/// Human-readable identity of this build.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// True only when the crate was compiled with `--features write_path`.
pub const WRITE_PATH_COMPILED: bool = cfg!(feature = "write_path");

/// Banner printed by the monitor.
pub fn banner() -> String {
    format!("X52 Integrity {VERSION}")
}

/// Convenience alias for the per-user data directory.
pub fn install_dir() -> std::path::PathBuf {
    paths::install_dir()
}
