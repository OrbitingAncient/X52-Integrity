//! In-process + on-disk log. The GUI reads this; driver helpers append too.

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[cfg(windows)]
use windows_sys::Win32::Foundation::SYSTEMTIME;
#[cfg(windows)]
use windows_sys::Win32::System::SystemInformation::GetLocalTime;

/// One log line.
#[derive(Debug, Clone)]
pub struct LogLine {
    /// Local `HH:MM:SS`.
    pub time: String,
    /// `info`, `warn`, `error`.
    pub level: &'static str,
    /// Message.
    pub text: String,
}

/// Shared ring buffer plus optional file.
#[derive(Clone)]
pub struct Logger {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    lines: VecDeque<LogLine>,
    file: Option<File>,
    file_path: Option<PathBuf>,
}

const MAX_LINES: usize = 800;

impl Logger {
    /// Create a logger. File logging is best-effort.
    pub fn new(file_path: Option<PathBuf>) -> Self {
        let file = file_path.as_ref().and_then(|p| {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            OpenOptions::new().create(true).append(true).open(p).ok()
        });
        Self {
            inner: Arc::new(Mutex::new(Inner {
                lines: VecDeque::new(),
                file,
                file_path,
            })),
        }
    }

    /// Path of the log file, if any.
    pub fn file_path(&self) -> Option<PathBuf> {
        self.inner.lock().ok().and_then(|g| g.file_path.clone())
    }

    /// Info.
    pub fn info(&self, text: impl Into<String>) {
        self.push("INFO", text.into());
    }

    /// Warning.
    pub fn warn(&self, text: impl Into<String>) {
        self.push("WARN", text.into());
    }

    /// Error.
    pub fn error(&self, text: impl Into<String>) {
        self.push("ERR", text.into());
    }

    /// Clear the in-memory ring. The file is left intact.
    pub fn clear(&self) {
        if let Ok(mut g) = self.inner.lock() {
            g.lines.clear();
        }
    }

    fn push(&self, level: &'static str, text: String) {
        let time = local_hms();
        if let Ok(mut g) = self.inner.lock() {
            if let Some(f) = g.file.as_mut() {
                let _ = writeln!(f, "{time}  {level:<4}  {text}");
                let _ = f.flush();
            }
            g.lines.push_back(LogLine { time, level, text });
            while g.lines.len() > MAX_LINES {
                g.lines.pop_front();
            }
        }
    }

    /// Snapshot of current lines (oldest first).
    pub fn snapshot(&self) -> Vec<LogLine> {
        self.inner
            .lock()
            .map(|g| g.lines.iter().cloned().collect())
            .unwrap_or_default()
    }
}

fn local_hms() -> String {
    #[cfg(windows)]
    unsafe {
        let mut st = std::mem::zeroed::<SYSTEMTIME>();
        GetLocalTime(&mut st);
        format!("{:02}:{:02}:{:02}", st.wHour, st.wMinute, st.wSecond)
    }
    #[cfg(not(windows))]
    {
        let mut t = 0i64;
        unsafe {
            libc::time(&mut t);
            let mut tm = std::mem::zeroed::<libc::tm>();
            libc::localtime_r(&t, &mut tm);
            format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_caps_at_800() {
        let log = Logger::new(None);
        for i in 0..900 {
            log.info(format!("{i}"));
        }
        let snap = log.snapshot();
        assert_eq!(snap.len(), 800);
        assert_eq!(snap[0].text, "100");
        assert_eq!(snap[799].text, "899");
        assert_eq!(snap[0].level, "INFO");
    }
}
