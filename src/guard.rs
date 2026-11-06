//! Exit / panic restore hook.
//!
//! In monitor-only mode this is a documented no-op: we never changed LEDs or
//! the MFD, so there is nothing to restore. If a future write-path session
//! issued vendor commands, this guard is the place that restores a known-safe
//! default (LED brightness mid, MFD brightness mid, no further commands).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Tracks whether this process issued any hardware write.
#[derive(Debug, Clone)]
pub struct ExitGuard {
    writes_issued: Arc<AtomicBool>,
}

impl ExitGuard {
    /// Create a guard that currently has no writes to reverse.
    pub fn new() -> Self {
        Self {
            writes_issued: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Record that a hardware write happened. The monitor never calls this.
    #[allow(dead_code)]
    pub fn mark_write_issued(&self) {
        self.writes_issued.store(true, Ordering::SeqCst);
    }

    /// True if a restore would attempt vendor commands.
    pub fn writes_issued(&self) -> bool {
        self.writes_issued.load(Ordering::SeqCst)
    }

    /// Restore LEDs/MFD if we changed them. Monitor-only: logs and returns.
    pub fn restore_now(&self) {
        if !self.writes_issued() {
            eprintln!("ExitGuard: no hardware writes were issued; nothing to restore.");
            return;
        }
        // Write-path restore would go here. We still refuse to send anything
        // unless a future SafetyGate allows it — and this deliverable's
        // monitor binary cannot arm that gate.
        eprintln!(
            "ExitGuard: writes were marked, but monitor-only restore cannot send vendor commands."
        );
    }
}

impl Default for ExitGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ExitGuard {
    fn drop(&mut self) {
        self.restore_now();
    }
}

/// Install a panic hook that runs restore before the default hook.
pub fn install_panic_restore(guard: ExitGuard) {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("panic: running ExitGuard before abort");
        guard.restore_now();
        default(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_session_restore_is_noop() {
        let g = ExitGuard::new();
        assert!(!g.writes_issued());
        g.restore_now();
    }
}
