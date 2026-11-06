//! Console monitor. Always monitor-only. Refuses write-related flags.

use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::diagnose;
use crate::error::Error;
use crate::guard::{install_panic_restore, ExitGuard};
use crate::hid::{list_all_hid, list_x52, pick_x52, HidReader};
use crate::identity::format_vid_pid;
use crate::live::{self, LiveDevice};
use crate::pipeline::AxisProcessing;
use crate::profile::Profile;
use crate::safety::SafetyGate;
use crate::state::{MonitorSnapshot, SafetySnapshot, X52State};

/// Parsed command line.
#[derive(Debug, Clone)]
pub struct Args {
    /// List matching X52 devices and exit.
    pub list: bool,
    /// List every HID device (still read-only).
    pub list_all_hid: bool,
    /// Print one snapshot and exit.
    pub once: bool,
    /// JSON instead of the dashboard.
    pub json: bool,
    /// Stop after N seconds.
    pub duration: Option<u64>,
    /// Optional profile path. Write flags in the profile are ignored.
    pub profile: Option<PathBuf>,
    /// Which X52 to open when several are present.
    pub index: usize,
    /// Extra logs.
    pub verbose: bool,
    /// Driver-stack diagnose (read-only).
    pub diagnose: bool,
}

impl Args {
    /// Parse argv. Unknown flags, and any write-sounding flags, are errors.
    pub fn parse<I, S>(args: I) -> Result<Self, Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut out = Args {
            list: false,
            list_all_hid: false,
            once: false,
            json: false,
            duration: None,
            profile: None,
            index: 0,
            verbose: false,
            diagnose: false,
        };
        let mut it = args.into_iter();
        while let Some(raw) = it.next() {
            let a = raw.as_ref();
            match a {
                "--list" | "-l" => out.list = true,
                "--list-all-hid" => out.list_all_hid = true,
                "--once" => out.once = true,
                "--json" => out.json = true,
                "--verbose" | "-v" => out.verbose = true,
                "--diagnose" => out.diagnose = true,
                "--help" | "-h" => return Err(Error::Usage(help_text())),
                "--index" => {
                    let v = it
                        .next()
                        .ok_or_else(|| Error::Usage("--index needs a number".into()))?;
                    out.index = v.as_ref().parse().map_err(|_| {
                        Error::Usage("--index must be a non-negative integer".into())
                    })?;
                }
                "--duration" => {
                    let v = it
                        .next()
                        .ok_or_else(|| Error::Usage("--duration needs seconds".into()))?;
                    out.duration = Some(
                        v.as_ref()
                            .parse()
                            .map_err(|_| Error::Usage("--duration must be seconds".into()))?,
                    );
                }
                "--profile" => {
                    let v = it
                        .next()
                        .ok_or_else(|| Error::Usage("--profile needs a path".into()))?;
                    out.profile = Some(PathBuf::from(v.as_ref()));
                }
                other if other.starts_with('-') => {
                    if looks_like_write_flag(other) {
                        return Err(Error::Usage(format!(
                            "refused flag '{other}': x52-monitor is MONITOR-ONLY and will not send vendor commands, flash firmware, or write a virtual joystick.\nRebuild with write_path and a later confirmed arming step if you want that — after we discuss it."
                        )));
                    }
                    return Err(Error::Usage(format!("unknown flag '{other}'. See --help.")));
                }
                other => {
                    return Err(Error::Usage(format!("unexpected argument '{other}'")));
                }
            }
        }
        Ok(out)
    }
}

fn looks_like_write_flag(s: &str) -> bool {
    let l = s.to_ascii_lowercase();
    l.contains("write")
        || l.contains("led")
        || l.contains("mfd")
        || l.contains("flash")
        || l.contains("firmware")
        || l.contains("vjoy")
        || l.contains("vigem")
        || l.contains("inject")
        || l.contains("macro")
        || l.contains("admin")
        || l.contains("elevate")
        || l.contains("arm")
}

fn help_text() -> String {
    format!(
        "{}\n\n\
Usage: x52-monitor [options]\n\
\n\
  --list              List X52 HID devices and exit (read-only)\n\
  --list-all-hid      List every HID device VID/PID and exit\n\
  --diagnose          Print driver stack + Memory Integrity (read-only)\n\
  --once              Print one decoded report and exit\n\
  --json              JSON output (with --once or --diagnose)\n\
  --duration N        Run the live dashboard for N seconds\n\
  --index N           Which X52 to open if several are present (default 0)\n\
  --profile PATH      Load JSON profile (write flags are ignored)\n\
  --verbose           Extra logs on stderr\n\
  --help              This text\n\
\n\
This binary never sends vendor control transfers, never flashes firmware,\n\
and never installs drivers. Ctrl+C exits; ExitGuard has nothing to restore\n\
because no hardware writes were issued.\n",
        crate::banner()
    )
}

/// Entry used by the `x52-monitor` binary.
pub fn run(args: Args) -> Result<(), Error> {
    let guard = ExitGuard::new();
    install_panic_restore(guard.clone());
    let _ctrl = CtrlC::install();

    let gate = SafetyGate::monitor_only();
    eprintln!("{}", crate::banner());

    if args.diagnose {
        let report = diagnose::collect()?;
        if args.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| Error::Io(e.to_string()))?
            );
        } else {
            print!("{}", diagnose::render(&report));
        }
        return Ok(());
    }

    if args.list_all_hid {
        let all = list_all_hid()?;
        println!("HID devices ({}):", all.len());
        for (i, d) in all.iter().enumerate() {
            println!("  [{i}] {}", d.summary_line());
        }
        return Ok(());
    }

    if args.list {
        let found = list_x52()?;
        if found.is_empty() {
            return Err(Error::DeviceNotFound);
        }
        println!("X52 devices ({}):", found.len());
        for (i, d) in found.iter().enumerate() {
            println!("  [{i}] {}", d.summary_line());
            if d.opened_with_write_access && args.verbose {
                eprintln!("    note: CreateFile required GENERIC_WRITE to open for reading; no writes were sent.");
            }
        }
        return Ok(());
    }

    let mut processing = AxisProcessing::default();
    if let Some(path) = &args.profile {
        let profile = Profile::load(path)?;
        if profile.requests_writes() {
            eprintln!(
                "warning: profile '{}' requests write capabilities; x52-monitor ignores them.",
                profile.name
            );
        }
        processing = profile.processing.clone();
        if args.verbose {
            eprintln!("loaded profile '{}' from {}", profile.name, path.display());
        }
    }

    let info = pick_x52(args.index)?;
    eprintln!(
        "opening {} {} product='{}'  (read-only)",
        info.kind.display_name(),
        format_vid_pid(info.vid, info.pid),
        info.product
    );
    if !info.upper_filters.is_empty() || !info.lower_filters.is_empty() {
        eprintln!(
            "driver filters present: upper={:?} lower={:?} service='{}'",
            info.upper_filters, info.lower_filters, info.service
        );
        eprintln!(
            "existing Logitech/Saitek software was left in place. No uninstall was performed."
        );
    }

    let mut reader = HidReader::open(info.clone())?;
    if reader.opened_with_write_access() {
        eprintln!("note: handle opened with GENERIC_WRITE as a read fallback; the safety gate still blocks every send.");
    }
    let live_dev = live::list_x52()?.into_iter().next();
    match &live_dev {
        Some(d) => eprintln!(
            "live input '{}' VID={:04X} PID={:04X} axes={} buttons={} (OS joystick API if HID interrupt is silent)",
            d.name, d.vid, d.pid, d.num_axes, d.num_buttons
        ),
        None => eprintln!("live input: no matching X52 (HID interrupt will be the only source)"),
    }

    let started = Instant::now();
    let mut report_number = 0u64;
    let mut last_state: Option<X52State> = None;
    let mut hid_interrupt_live = true;
    let deadline = args.duration.map(|s| started + Duration::from_secs(s));

    if args.once {
        let state = wait_one_report(
            &mut reader,
            live_dev.as_ref(),
            &processing,
            &mut report_number,
            started,
            &mut hid_interrupt_live,
        )?;
        emit_once(&args, &gate, &reader, &state)?;
        return Ok(());
    }

    enable_vt();
    print!("\x1b[?1049h\x1b[2J");
    let _alt = AltScreen;
    let mut last_draw = Instant::now() - Duration::from_secs(1);
    loop {
        if CtrlC::tripped() {
            break;
        }
        if let Some(t) = deadline {
            if Instant::now() >= t {
                break;
            }
        }
        match read_best(
            &mut reader,
            live_dev.as_ref(),
            &processing,
            report_number + 1,
            started.elapsed().as_millis() as u64,
            &mut hid_interrupt_live,
        ) {
            Ok(Some(state)) => {
                report_number = state.report_number;
                last_state = Some(state);
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!("read error: {e}");
                std::thread::sleep(Duration::from_millis(250));
            }
        }
        if last_draw.elapsed() >= Duration::from_millis(80) {
            if let Some(state) = &last_state {
                let frame = render_dashboard(&gate, reader.info(), state, started.elapsed());
                print!("\x1b[H{frame}");
                let _ = io::stdout().flush();
            } else {
                println!(
                    "\x1b[Hwaiting for first HID report from {}…",
                    reader.info().product
                );
                let _ = io::stdout().flush();
            }
            last_draw = Instant::now();
        }
    }
    Ok(())
}

fn wait_one_report(
    reader: &mut HidReader,
    mm: Option<&LiveDevice>,
    processing: &AxisProcessing,
    report_number: &mut u64,
    started: Instant,
    hid_interrupt_live: &mut bool,
) -> Result<X52State, Error> {
    eprintln!("waiting up to 8s for stick state (HID interrupt, then OS joystick API)");
    let give_up = Instant::now() + Duration::from_secs(8);
    while Instant::now() < give_up {
        if CtrlC::tripped() {
            return Err(Error::Io("cancelled".into()));
        }
        if let Some(state) = read_best(
            reader,
            mm,
            processing,
            *report_number + 1,
            started.elapsed().as_millis() as u64,
            hid_interrupt_live,
        )? {
            *report_number = state.report_number;
            return Ok(state);
        }
    }
    Err(Error::Io(
        "timed out waiting for stick state from HID and the OS joystick API".into(),
    ))
}

fn read_best(
    reader: &mut HidReader,
    mm: Option<&LiveDevice>,
    processing: &AxisProcessing,
    report_number: u64,
    elapsed_ms: u64,
    hid_interrupt_live: &mut bool,
) -> Result<Option<X52State>, Error> {
    if *hid_interrupt_live {
        match reader.read_state(processing, report_number, elapsed_ms)? {
            Some(state) if looks_like_live_hid(&state) => return Ok(Some(state)),
            _ => {
                *hid_interrupt_live = false;
                eprintln!("HID interrupt not delivering reports; using OS joystick API for live axes/buttons");
            }
        }
    }
    if let Some(dev) = mm {
        let mut state = live::read_state(dev, report_number, elapsed_ms)?;
        state.apply_pipeline(processing);
        return Ok(Some(state));
    }
    Ok(None)
}

fn looks_like_live_hid(state: &X52State) -> bool {
    // HidD_GetInputReport on this firmware often returns a near-zero buffer.
    // A live interrupt report has more than one non-zero byte, or a centered axis.
    let nonzero = state.raw.iter().filter(|b| **b != 0).count();
    if nonzero > 2 {
        return true;
    }
    state
        .axes
        .iter()
        .any(|a| a.centered && a.raw > 64 && (a.logical_max as i64 - a.raw as i64) > 64)
}

fn emit_once(
    args: &Args,
    gate: &SafetyGate,
    reader: &HidReader,
    state: &X52State,
) -> Result<(), Error> {
    if args.json {
        let snap = MonitorSnapshot {
            banner: crate::banner(),
            product: reader.info().product.clone(),
            vid: reader.info().vid,
            pid: reader.info().pid,
            kind: reader.info().kind.display_name().to_string(),
            path: reader.info().path.clone(),
            driver_service: reader.info().service.clone(),
            upper_filters: reader.info().upper_filters.clone(),
            lower_filters: reader.info().lower_filters.clone(),
            safety: SafetySnapshot::from(gate.snapshot()),
            state: Some(state.clone()),
        };
        let _ = io::stderr().flush();
        println!(
            "{}",
            serde_json::to_string_pretty(&snap).map_err(|e| Error::Io(e.to_string()))?
        );
    } else {
        println!(
            "{}",
            render_dashboard(
                gate,
                reader.info(),
                state,
                Duration::from_millis(state.elapsed_ms)
            )
        );
    }
    Ok(())
}

fn render_dashboard(
    gate: &SafetyGate,
    info: &crate::hid::HidDeviceInfo,
    state: &X52State,
    elapsed: Duration,
) -> String {
    let mut s = String::new();
    s.push_str(
        "╔══════════════════════════════════════════════════════════════════════════════╗\n",
    );
    s.push_str(&format!("║ {:<76} ║\n", crate::banner()));
    s.push_str(&format!(
        "║ {:<76} ║\n",
        format!(
            "{}  {}  product='{}'",
            info.kind.display_name(),
            format_vid_pid(info.vid, info.pid),
            truncate(&info.product, 40)
        )
    ));
    s.push_str(&format!(
        "║ safety: {:<67} ║\n",
        truncate(&gate.snapshot().line(), 67)
    ));
    s.push_str(&format!(
        "║ driver: service='{}'  upper={:?}  lower={:?}\n",
        truncate(&info.service, 20),
        info.upper_filters,
        info.lower_filters
    ));
    s.push_str(
        "╠══════════════════════════════════════════════════════════════════════════════╣\n",
    );
    s.push_str(&format!(
        "║ report #{:<8}  {:>5} bytes  elapsed {:>7.2}s  mode {:<6}                 ║\n",
        state.report_number,
        state.raw.len(),
        elapsed.as_secs_f32(),
        state.mode.as_str()
    ));
    s.push_str(
        "║ Axes                                                                         ║\n",
    );
    for axis in &state.axes {
        let bar = bar(axis.processed, axis.centered);
        s.push_str(&format!(
            "║  {:<10} {bar} raw={:<5} n={:>+6.3} p={:>+6.3}                 \n",
            truncate(&axis.name, 10),
            axis.raw,
            axis.normalized,
            axis.processed
        ));
    }
    if state.axes.is_empty() {
        s.push_str(
            "║  (no value caps decoded — raw hex below)                                     ║\n",
        );
    }
    s.push_str(
        "║ Hats                                                                         ║\n",
    );
    if state.hats.is_empty() {
        s.push_str(
            "║  (none)                                                                      ║\n",
        );
    } else {
        for hat in &state.hats {
            s.push_str(&format!(
                "║  {:<10} {:>6}  raw={}                                                    \n",
                truncate(&hat.name, 10),
                hat.direction,
                hat.raw
            ));
        }
    }
    s.push_str(
        "║ Buttons pressed                                                              ║\n",
    );
    if state.buttons_pressed.is_empty() {
        s.push_str(
            "║  (none)                                                                      ║\n",
        );
    } else {
        let names: Vec<String> = state
            .button_names
            .iter()
            .cloned()
            .zip(state.buttons_pressed.iter())
            .map(|(n, _)| n)
            .collect();
        let line = names.join("  ");
        s.push_str(&format!("║  {} \n", truncate(&line, 74)));
    }
    s.push_str(&format!("║ raw {:<73} \n", hex_preview(&state.raw, 60)));
    s.push_str(
        "╚══════════════════════════════════════════════════════════════════════════════╝\n",
    );
    s.push_str("Ctrl+C quit   MONITOR-ONLY   no vendor commands   no virtual joystick writes\n");
    s
}

fn bar(v: f64, centered: bool) -> String {
    const W: usize = 21;
    let mut chars = vec!['-'; W];
    if centered {
        let mid = W / 2;
        chars[mid] = '|';
        let x = ((v + 1.0) * 0.5 * (W as f64 - 1.0)).round() as isize;
        let x = x.clamp(0, (W - 1) as isize) as usize;
        chars[x] = '#';
    } else {
        let n = (v.clamp(0.0, 1.0) * (W as f64)).round() as usize;
        for item in chars.iter_mut().take(n.min(W)) {
            *item = '#';
        }
    }
    format!("[{}]", chars.into_iter().collect::<String>())
}

fn hex_preview(bytes: &[u8], max: usize) -> String {
    let take = bytes.len().min(max / 3);
    let mut s = bytes[..take]
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ");
    if bytes.len() > take {
        s.push('…');
    }
    s
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let t: String = s.chars().take(n.saturating_sub(1)).collect();
        format!("{t}…")
    }
}

fn enable_vt() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Console::{
            GetConsoleMode, GetStdHandle, SetConsoleMode, STD_OUTPUT_HANDLE,
        };
        unsafe {
            let h = GetStdHandle(STD_OUTPUT_HANDLE);
            let mut mode = 0u32;
            if GetConsoleMode(h, &mut mode) != 0 {
                const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
                let _ = SetConsoleMode(h, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
            }
        }
    }
}

struct AltScreen;
impl Drop for AltScreen {
    fn drop(&mut self) {
        print!("\x1b[?1049l");
        let _ = io::stdout().flush();
    }
}

struct CtrlC;

impl CtrlC {
    fn install() -> Self {
        #[cfg(windows)]
        unsafe {
            let _ = windows_sys::Win32::System::Console::SetConsoleCtrlHandler(
                Some(ctrl_handler_win),
                1,
            );
        }
        #[cfg(unix)]
        unsafe {
            libc::signal(libc::SIGINT, ctrl_handler_unix as libc::sighandler_t);
        }
        Self
    }

    fn tripped() -> bool {
        STOP.load(std::sync::atomic::Ordering::SeqCst)
    }
}

static STOP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(windows)]
unsafe extern "system" fn ctrl_handler_win(_ctrl: u32) -> windows_sys::Win32::Foundation::BOOL {
    STOP.store(true, std::sync::atomic::Ordering::SeqCst);
    1
}

#[cfg(unix)]
extern "C" fn ctrl_handler_unix(_sig: i32) {
    STOP.store(true, std::sync::atomic::Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_flags_are_refused() {
        for flag in [
            "--led",
            "--flash",
            "--firmware",
            "--vjoy",
            "--arm",
            "--admin",
        ] {
            let err = Args::parse([flag]).unwrap_err();
            let msg = err.to_string();
            assert!(msg.contains("MONITOR-ONLY"), "{flag}: {msg}");
        }
    }

    #[test]
    fn list_and_once_parse() {
        let a = Args::parse(["--list", "--once", "--json", "--index", "1"]).unwrap();
        assert!(a.list && a.once && a.json);
        assert_eq!(a.index, 1);
    }
}
