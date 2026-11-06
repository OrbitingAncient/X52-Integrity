//! Device manager window.
#![allow(
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::needless_pass_by_value,
    clippy::struct_excessive_bools,
    clippy::map_unwrap_or,
    clippy::unchecked_time_subtraction,
    clippy::single_match_else,
    clippy::redundant_closure_for_method_calls,
    clippy::uninlined_format_args
)]

use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Color32, CornerRadius, FontId, Frame, Label, Layout, Margin, Rect, RichText,
    Sense, Stroke, Vec2,
};
use x52_integrity::config::{AppConfig, CalibrationSnapshot};
use x52_integrity::diagnose;
use x52_integrity::install;
use x52_integrity::live::{self, LiveDevice};
use x52_integrity::logbuf::Logger;
use x52_integrity::paths;
#[cfg(windows)]
use x52_integrity::setup;
use x52_integrity::state::X52State;

const BG: Color32 = Color32::from_rgb(18, 20, 24);
const PANEL: Color32 = Color32::from_rgb(24, 26, 31);
const CARD: Color32 = Color32::from_rgb(32, 35, 42);
const STROKE: Color32 = Color32::from_rgb(52, 56, 64);
const TEXT: Color32 = Color32::from_rgb(226, 229, 234);
const MUTED: Color32 = Color32::from_rgb(132, 138, 148);
const ACCENT: Color32 = Color32::from_rgb(74, 130, 214);
const OK: Color32 = Color32::from_rgb(62, 176, 122);
const WARN: Color32 = Color32::from_rgb(210, 158, 62);
const ERR: Color32 = Color32::from_rgb(214, 90, 90);
const TRACK: Color32 = Color32::from_rgb(16, 17, 20);

pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 720.0])
            .with_min_inner_size([900.0, 580.0])
            .with_title("X52 Integrity"),
        hardware_acceleration: eframe::HardwareAcceleration::Preferred,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "X52 Integrity",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

struct App {
    log: Logger,
    config: AppConfig,
    status: Status,
    live: Option<X52State>,
    live_dev: Option<LiveDevice>,
    started: Instant,
    last_poll: Instant,
    last_scan: Instant,
    last_status: Instant,
    report_n: u64,
    busy: Option<String>,
    confirm: Option<Confirm>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    close_after: Option<Instant>,
    status_inflight: bool,
}

#[derive(Clone)]
struct Status {
    elevated: bool,
    memory_integrity: Option<bool>,
    app_installed: bool,
    from_install: bool,
    install_dir: String,
    log_path: String,
    logitech_backup: bool,
    x52: Vec<DeviceRow>,
    live_count: usize,
    error: Option<String>,
}

#[derive(Clone)]
struct DeviceRow {
    kind: String,
    vid: u16,
    pid: u16,
    product: String,
    service: String,
    upper: Vec<String>,
    lower: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Confirm {
    InstallApp,
    RemoveApp,
    InboxHid,
    RestoreLogitech,
}

enum Msg {
    Status(Status),
    StatusFailed(String),
    Busy(Option<String>),
    CloseSoon,
}

impl Default for Status {
    fn default() -> Self {
        Self {
            elevated: diagnose::is_elevated(),
            memory_integrity: None,
            app_installed: paths::app_installed(),
            from_install: paths::running_from_install_dir(),
            install_dir: paths::install_dir().display().to_string(),
            log_path: paths::log_path().display().to_string(),
            logitech_backup: logitech_backup_present(),
            x52: Vec::new(),
            live_count: 0,
            error: None,
        }
    }
}

fn logitech_backup_present() -> bool {
    #[cfg(windows)]
    {
        setup::logitech_inf().is_some()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn collect_status() -> Result<Status, String> {
    let report = diagnose::collect().map_err(|e| e.to_string())?;
    let live_count = live::list_x52().map(|v| v.len()).unwrap_or(0);
    let x52 = report
        .x52_devices
        .iter()
        .map(|d| DeviceRow {
            kind: d.kind.clone(),
            vid: d.vid,
            pid: d.pid,
            product: d.product.clone(),
            service: d.service.clone(),
            upper: d.upper_filters.clone(),
            lower: d.lower_filters.clone(),
        })
        .collect();
    Ok(Status {
        elevated: report.elevated,
        memory_integrity: report.memory_integrity,
        app_installed: paths::app_installed(),
        from_install: paths::running_from_install_dir(),
        install_dir: paths::install_dir().display().to_string(),
        log_path: paths::log_path().display().to_string(),
        logitech_backup: logitech_backup_present(),
        x52,
        live_count,
        error: None,
    })
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_theme(&cc.egui_ctx);
        let log = Logger::new(Some(paths::log_path()));
        log.info(format!("Started v{}", x52_integrity::VERSION));
        let (tx, rx) = mpsc::channel();
        let app = Self {
            log,
            config: AppConfig::load(),
            status: Status::default(),
            live: None,
            live_dev: None,
            started: Instant::now(),
            last_poll: Instant::now() - Duration::from_secs(1),
            last_scan: Instant::now() - Duration::from_secs(5),
            last_status: Instant::now() - Duration::from_secs(5),
            report_n: 0,
            busy: None,
            confirm: None,
            tx,
            rx,
            close_after: None,
            status_inflight: true,
        };
        app.queue_status();
        app
    }

    fn queue_status(&self) {
        let tx = self.tx.clone();
        std::thread::spawn(move || match collect_status() {
            Ok(s) => {
                let _ = tx.send(Msg::Status(s));
            }
            Err(e) => {
                let _ = tx.send(Msg::StatusFailed(e));
            }
        });
    }

    fn drain_msgs(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Status(s) => {
                    self.status = s;
                    self.status_inflight = false;
                    self.last_status = Instant::now();
                }
                Msg::StatusFailed(e) => {
                    self.status.error = Some(e.clone());
                    self.status_inflight = false;
                    self.log.error(e);
                }
                Msg::Busy(b) => self.busy = b,
                Msg::CloseSoon => {
                    self.close_after = Some(Instant::now() + Duration::from_millis(800));
                }
            }
        }
    }

    fn poll_live(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.last_scan) > Duration::from_secs(2) || self.live_dev.is_none() {
            self.last_scan = now;
            self.live_dev = live::list_x52().ok().and_then(|v| v.into_iter().next());
        }
        if now.duration_since(self.last_poll) < Duration::from_millis(40) {
            return;
        }
        self.last_poll = now;
        let Some(dev) = self.live_dev.clone() else {
            self.live = None;
            return;
        };
        let elapsed = self.started.elapsed().as_millis() as u64;
        match live::read_state(&dev, self.report_n, elapsed) {
            Ok(st) => {
                self.report_n += 1;
                self.live = Some(st);
            }
            Err(_) => {
                self.live = None;
                self.live_dev = None;
            }
        }
    }

    fn finish_first_run(&mut self) {
        if let Some(st) = &self.live {
            self.config.calibration = Some(CalibrationSnapshot {
                x: axis_raw(st, "X"),
                y: axis_raw(st, "Y"),
                z: axis_raw(st, "Z"),
                r: axis_raw(st, "R"),
            });
        }
        self.config.first_run_complete = true;
        if let Err(e) = self.config.save() {
            self.log.warn(format!("Could not save config: {e}"));
        } else {
            self.log.info("First-run complete");
        }
    }

    fn start_test(&mut self) {
        if self.busy.is_some() {
            return;
        }
        self.busy = Some("Testing".into());
        let log = self.log.clone();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            log.info("Device test started");
            match diagnose::collect() {
                Ok(r) => {
                    for line in diagnose::render(&r).lines() {
                        if !line.is_empty() {
                            log.info(line);
                        }
                    }
                }
                Err(e) => log.error(format!("Diagnose failed: {e}")),
            }
            match live::list_x52() {
                Ok(devs) if devs.is_empty() => log.warn("No joystick report from the OS"),
                Ok(devs) => {
                    for d in &devs {
                        match live::read_state(d, 0, 0) {
                            Ok(st) => {
                                let axes = st
                                    .axes
                                    .iter()
                                    .map(|a| format!("{}={}", a.name, a.raw))
                                    .collect::<Vec<_>>()
                                    .join("  ");
                                let hat =
                                    st.hats.first().map(|h| h.direction.as_str()).unwrap_or("—");
                                let buttons = if st.buttons_pressed.is_empty() {
                                    "none".into()
                                } else {
                                    st.buttons_pressed
                                        .iter()
                                        .map(|b| b.to_string())
                                        .collect::<Vec<_>>()
                                        .join(",")
                                };
                                log.info(format!("Input  {axes}  hat={hat}  buttons={buttons}"));
                            }
                            Err(e) => log.error(format!("Input read failed: {e}")),
                        }
                    }
                }
                Err(e) => log.error(format!("Input list failed: {e}")),
            }
            log.info("Device test complete");
            let _ = tx.send(Msg::Busy(None));
            match collect_status() {
                Ok(s) => {
                    let _ = tx.send(Msg::Status(s));
                }
                Err(e) => {
                    let _ = tx.send(Msg::StatusFailed(e));
                }
            }
        });
    }

    fn run_confirmed(&mut self, kind: Confirm) {
        if self.busy.is_some() {
            return;
        }
        let label = match kind {
            Confirm::InstallApp => "Installing",
            Confirm::RemoveApp => "Removing",
            Confirm::InboxHid => "Updating driver",
            Confirm::RestoreLogitech => "Restoring driver",
        };
        self.busy = Some(label.into());
        self.confirm = None;
        let log = self.log.clone();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let result = match kind {
                Confirm::InstallApp => install::install_app_detailed(Some(&log)).map(|r| {
                    for w in &r.warnings {
                        log.warn(w);
                    }
                    r.message
                }),
                Confirm::RemoveApp => install::uninstall_app(),
                Confirm::InboxHid => driver_action(true),
                Confirm::RestoreLogitech => driver_action(false),
            };
            match result {
                Ok(msg) => {
                    log.info(&msg);
                    if kind == Confirm::RemoveApp && paths::running_from_install_dir() {
                        let _ = tx.send(Msg::CloseSoon);
                    }
                }
                Err(e) => log.error(e.to_string()),
            }
            let _ = tx.send(Msg::Busy(None));
            match collect_status() {
                Ok(s) => {
                    let _ = tx.send(Msg::Status(s));
                }
                Err(e) => {
                    let _ = tx.send(Msg::StatusFailed(e));
                }
            }
        });
    }
}

fn driver_action(inbox: bool) -> Result<String, x52_integrity::Error> {
    #[cfg(windows)]
    {
        setup::run_driver_action_from_gui(inbox)
    }
    #[cfg(not(windows))]
    {
        let _ = inbox;
        Err(x52_integrity::Error::Platform(
            "driver bind is Windows-only".into(),
        ))
    }
}

fn axis_raw(st: &X52State, name: &str) -> u32 {
    st.axes
        .iter()
        .find(|a| a.name == name)
        .map(|a| a.raw)
        .unwrap_or(0)
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_msgs();
        self.poll_live();
        if !self.status_inflight
            && self.busy.is_none()
            && self.last_status.elapsed() > Duration::from_secs(3)
        {
            self.status_inflight = true;
            self.queue_status();
        }
        if let Some(when) = self.close_after {
            if Instant::now() >= when {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        ctx.request_repaint_after(Duration::from_millis(50));

        egui::TopBottomPanel::top("header")
            .exact_height(72.0)
            .frame(Frame {
                inner_margin: Margin::symmetric(20, 14),
                fill: PANEL,
                stroke: Stroke::new(1.0_f32, STROKE),
                ..Default::default()
            })
            .show(ctx, |ui| {
                draw_header(ui, &self.status, self.busy.as_deref());
            });

        egui::TopBottomPanel::bottom("actions")
            .exact_height(56.0)
            .frame(Frame {
                inner_margin: Margin::symmetric(16, 12),
                fill: PANEL,
                stroke: Stroke::new(1.0_f32, STROKE),
                ..Default::default()
            })
            .show(ctx, |ui| {
                draw_actions(ui, self);
            });

        egui::SidePanel::right("activity")
            .resizable(true)
            .default_width(400.0)
            .min_width(300.0)
            .frame(Frame {
                inner_margin: Margin::same(16),
                fill: BG,
                ..Default::default()
            })
            .show(ctx, |ui| {
                draw_activity(ui, &self.log, &self.status.log_path);
            });

        egui::CentralPanel::default()
            .frame(Frame {
                inner_margin: Margin::same(16),
                fill: BG,
                ..Default::default()
            })
            .show(ctx, |ui| {
                draw_device(ui, &self.status, self.live_dev.as_ref(), self.live.as_ref());
            });

        if !self.config.first_run_complete {
            draw_first_run(ctx, self);
        }

        let mut accepted = None;
        if let Some(kind) = self.confirm {
            draw_confirm(ctx, kind, &mut self.confirm, |k| accepted = Some(k));
        }
        if let Some(k) = accepted {
            self.run_confirmed(k);
        }
    }
}

fn apply_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = CARD;
    visuals.window_stroke = Stroke::new(1.0_f32, STROKE);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(46, 50, 58);
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(46, 50, 58);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(58, 64, 74);
    visuals.widgets.active.bg_fill = Color32::from_rgb(66, 74, 86);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(5);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(5);
    visuals.widgets.active.corner_radius = CornerRadius::same(5);
    visuals.selection.bg_fill = ACCENT;
    visuals.override_text_color = Some(TEXT);
    visuals.extreme_bg_color = TRACK;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(10.0, 7.0);
    style.spacing.button_padding = Vec2::new(12.0, 6.0);
    ctx.set_style(style);
}

fn card() -> Frame {
    Frame {
        inner_margin: Margin::same(16),
        corner_radius: CornerRadius::same(8),
        fill: CARD,
        stroke: Stroke::new(1.0_f32, STROKE),
        ..Default::default()
    }
}

fn draw_header(ui: &mut egui::Ui, st: &Status, busy: Option<&str>) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(
                RichText::new("X52 Integrity")
                    .size(20.0)
                    .color(TEXT)
                    .strong(),
            );
            let subtitle = if let Some(d) = st.x52.first() {
                let name = if d.product.is_empty() {
                    d.kind.as_str()
                } else {
                    d.product.as_str()
                };
                format!(
                    "{name}    {:04X}:{:04X}    {}",
                    d.vid,
                    d.pid,
                    driver_label(d)
                )
            } else {
                "No device connected".into()
            };
            ui.label(RichText::new(subtitle).size(13.0).color(MUTED));
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(format!("v{}", x52_integrity::VERSION))
                    .size(12.0)
                    .color(MUTED),
            );
            ui.add_space(12.0);
            if let Some(b) = busy {
                ui.label(RichText::new(b).size(13.0).color(ACCENT));
            } else {
                let connected = !st.x52.is_empty() && st.live_count > 0;
                let (dot, label) = if connected {
                    (OK, "Connected")
                } else if !st.x52.is_empty() {
                    (WARN, "No input")
                } else {
                    (MUTED, "Disconnected")
                };
                ui.label(RichText::new(label).size(13.0).color(dot));
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                ui.painter().circle_filled(rect.center(), 4.0, dot);
            }
        });
    });
}

fn driver_label(d: &DeviceRow) -> String {
    let mut filters = d.upper.clone();
    filters.extend(d.lower.iter().cloned());
    filters.retain(|f| !f.is_empty());
    if !filters.is_empty() {
        format!("Logitech  {}", filters.join(", "))
    } else if d.service.is_empty()
        || d.service.eq_ignore_ascii_case("hidusb")
        || d.service.eq_ignore_ascii_case("evdev")
    {
        if cfg!(windows) {
            "Windows HID".into()
        } else {
            "evdev".into()
        }
    } else {
        d.service.clone()
    }
}

fn draw_device(ui: &mut egui::Ui, st: &Status, dev: Option<&LiveDevice>, state: Option<&X52State>) {
    ui.spacing_mut().item_spacing.y = 12.0;
    card().show(ui, |ui| {
        section_title(ui, "Input");
        match (dev, state) {
            (Some(_), Some(stt)) => {
                egui::Grid::new("axes")
                    .num_columns(1)
                    .spacing([0.0, 8.0])
                    .show(ui, |ui| {
                        for a in &stt.axes {
                            axis_bar(ui, axis_label(&a.name), a.processed, a.centered, a.raw);
                            ui.end_row();
                        }
                    });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    kv_inline(
                        ui,
                        "Hat",
                        stt.hats
                            .first()
                            .map(|h| h.direction.as_str())
                            .unwrap_or("—"),
                    );
                    ui.add_space(18.0);
                    kv_inline(ui, "Mode", display_mode(stt.mode.as_str()));
                });
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Buttons").size(12.0).color(MUTED));
                    ui.add_space(8.0);
                    if stt.buttons_pressed.is_empty() {
                        ui.label(RichText::new("None").size(13.0).color(MUTED));
                    } else {
                        for b in &stt.buttons_pressed {
                            button_pill(ui, *b);
                        }
                    }
                });
            }
            _ => {
                ui.label(
                    RichText::new("Connect the throttle USB cable.")
                        .size(13.0)
                        .color(MUTED),
                );
            }
        }
    });

    card().show(ui, |ui| {
        section_title(ui, "System");
        let driver = st
            .x52
            .first()
            .map(driver_label)
            .unwrap_or_else(|| "—".into());
        kv(ui, "Driver", &driver);
        kv(
            ui,
            "Privilege",
            if st.elevated {
                if cfg!(windows) {
                    "Administrator"
                } else {
                    "root"
                }
            } else {
                "Standard user"
            },
        );
        if cfg!(windows) {
            kv(
                ui,
                "Memory Integrity",
                match st.memory_integrity {
                    Some(true) => "On",
                    Some(false) => "Off",
                    None => "Unknown",
                },
            );
        }
        kv(
            ui,
            "Install",
            if st.app_installed {
                if st.from_install {
                    "This copy"
                } else {
                    "Installed"
                }
            } else {
                "Not installed"
            },
        );
        if cfg!(windows) {
            kv(
                ui,
                "Backup",
                if st.logitech_backup {
                    "Logitech package present"
                } else {
                    "Missing"
                },
            );
        }
        ui.add_space(4.0);
        ui.label(
            RichText::new(&st.install_dir)
                .size(11.5)
                .color(MUTED)
                .monospace(),
        );
        if let Some(err) = &st.error {
            ui.add_space(6.0);
            ui.label(RichText::new(err).color(ERR).size(13.0));
        }
    });
}

fn display_mode(mode: &str) -> &str {
    match mode {
        "unknown" => "—",
        other => other,
    }
}

fn axis_label(name: &str) -> &str {
    match name {
        "X" => "Stick X",
        "Y" => "Stick Y",
        "Z" => "Throttle",
        "R" => "Twist",
        "U" => "Rotary E",
        "V" => "Rotary I",
        other => other,
    }
}

fn section_title(ui: &mut egui::Ui, title: &str) {
    ui.label(
        RichText::new(title.to_uppercase())
            .size(11.0)
            .color(MUTED)
            .strong(),
    );
    ui.add_space(8.0);
}

fn kv(ui: &mut egui::Ui, key: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [130.0, 18.0],
            Label::new(RichText::new(key).size(12.5).color(MUTED)),
        );
        ui.label(RichText::new(value).size(13.0).color(TEXT));
    });
}

fn kv_inline(ui: &mut egui::Ui, key: &str, value: &str) {
    ui.label(RichText::new(key).size(12.0).color(MUTED));
    ui.add_space(6.0);
    ui.label(RichText::new(value).size(13.0).color(TEXT).strong());
}

fn button_pill(ui: &mut egui::Ui, n: u16) {
    let galley =
        ui.painter()
            .layout_no_wrap(n.to_string(), FontId::proportional(12.0), Color32::WHITE);
    let size = Vec2::new((galley.size().x + 12.0).max(24.0), 20.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(4), ACCENT);
    ui.painter().galley(
        egui::pos2(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        Color32::WHITE,
    );
}

fn axis_bar(ui: &mut egui::Ui, name: &str, processed: f64, centered: bool, raw: u32) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [78.0, 18.0],
            Label::new(RichText::new(name).size(12.5).color(MUTED)),
        );
        let width = (ui.available_width() - 56.0).clamp(160.0, 420.0);
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 14.0), Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, CornerRadius::same(3), TRACK);
        if centered {
            let mid = rect.center().x;
            let t = processed.clamp(-1.0, 1.0) as f32;
            let x = mid + t * (rect.width() / 2.0 - 2.0);
            let bar = if t >= 0.0 {
                Rect::from_x_y_ranges(mid..=x, rect.y_range().shrink(2.0))
            } else {
                Rect::from_x_y_ranges(x..=mid, rect.y_range().shrink(2.0))
            };
            painter.rect_filled(bar, CornerRadius::same(2), ACCENT);
            painter.line_segment(
                [
                    egui::pos2(mid, rect.top() + 1.0),
                    egui::pos2(mid, rect.bottom() - 1.0),
                ],
                Stroke::new(1.0_f32, Color32::from_rgb(80, 86, 96)),
            );
        } else {
            let t = processed.clamp(0.0, 1.0) as f32;
            let w = (rect.width() - 2.0) * t;
            let bar = Rect::from_min_size(
                rect.min + Vec2::new(1.0, 2.0),
                Vec2::new(w.max(0.0), rect.height() - 4.0),
            );
            painter.rect_filled(bar, CornerRadius::same(2), OK);
        }
        if resp.hovered() {
            resp.on_hover_text(format!("{:+.3}", processed));
        }
        ui.add_sized(
            [52.0, 18.0],
            Label::new(
                RichText::new(format!("{raw:>5}"))
                    .monospace()
                    .size(12.0)
                    .color(MUTED),
            ),
        );
    });
}

fn draw_activity(ui: &mut egui::Ui, log: &Logger, path: &str) {
    ui.horizontal(|ui| {
        section_title(ui, "Activity");
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .add(egui::Button::new(RichText::new("Clear").size(12.0)))
                .clicked()
            {
                log.clear();
            }
            if ui
                .add(egui::Button::new(RichText::new("Folder").size(12.0)))
                .clicked()
            {
                match install::open_folder(&paths::log_path()) {
                    Ok(()) => log.info("Opened log folder"),
                    Err(e) => log.error(e.to_string()),
                }
            }
        });
    });
    ui.label(RichText::new(path).size(11.0).color(MUTED).monospace());
    ui.add_space(8.0);
    let lines = log.snapshot();
    egui::Frame {
        inner_margin: Margin::same(10),
        corner_radius: CornerRadius::same(6),
        fill: TRACK,
        stroke: Stroke::new(1.0_f32, STROKE),
        ..Default::default()
    }
    .show(ui, |ui| {
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.style_mut().spacing.item_spacing.y = 3.0;
                if lines.is_empty() {
                    ui.label(RichText::new("No events yet.").size(12.5).color(MUTED));
                }
                for line in &lines {
                    let color = match line.level {
                        "ERR" => ERR,
                        "WARN" => WARN,
                        _ => TEXT,
                    };
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new(&line.time)
                                .monospace()
                                .size(11.5)
                                .color(MUTED),
                        );
                        ui.label(
                            RichText::new(line.level)
                                .monospace()
                                .size(11.5)
                                .color(color)
                                .strong(),
                        );
                        ui.add(
                            Label::new(RichText::new(&line.text).size(12.5).color(color)).wrap(),
                        );
                    });
                }
            });
    });
}

fn draw_actions(ui: &mut egui::Ui, app: &mut App) {
    let enabled = app.busy.is_none() && app.confirm.is_none();
    ui.add_enabled_ui(enabled, |ui| {
        ui.horizontal(|ui| {
            let test = ui.add(
                egui::Button::new(RichText::new("Test").color(Color32::WHITE).strong())
                    .fill(ACCENT)
                    .min_size(Vec2::new(88.0, 28.0)),
            );
            if test.clicked() {
                app.start_test();
            }
            if ui
                .add(egui::Button::new("Install").min_size(Vec2::new(80.0, 28.0)))
                .clicked()
            {
                app.confirm = Some(Confirm::InstallApp);
            }
            if ui
                .add(egui::Button::new("Uninstall").min_size(Vec2::new(88.0, 28.0)))
                .clicked()
            {
                app.confirm = Some(Confirm::RemoveApp);
            }
            #[cfg(windows)]
            {
                ui.separator();
                if ui
                    .add(egui::Button::new("Windows HID").min_size(Vec2::new(108.0, 28.0)))
                    .on_hover_text("Bind the X52 to the inbox Windows HID driver")
                    .clicked()
                {
                    app.confirm = Some(Confirm::InboxHid);
                }
                if ui
                    .add(egui::Button::new("Logitech driver").min_size(Vec2::new(124.0, 28.0)))
                    .on_hover_text("Restore the saved Logitech package")
                    .clicked()
                {
                    app.confirm = Some(Confirm::RestoreLogitech);
                }
            }
        });
    });
}

fn draw_first_run(ctx: &egui::Context, app: &mut App) {
    egui::Window::new("Welcome")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .frame(Frame {
            inner_margin: Margin::same(18),
            corner_radius: CornerRadius::same(8),
            fill: CARD,
            stroke: Stroke::new(1.0_f32, STROKE),
            ..Default::default()
        })
        .show(ctx, |ui| {
            ui.set_max_width(460.0);
            ui.label(
                RichText::new("X52 Integrity")
                    .size(18.0)
                    .color(TEXT)
                    .strong(),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "Connect the throttle USB cable. Stick and throttle share that plug. Leave Memory Integrity on. This app runs as a normal user.",
                )
                .size(14.0)
                .color(TEXT),
            );
            ui.add_space(10.0);
            if let Some(st) = &app.live {
                ui.label(
                    RichText::new(format!(
                        "Live input OK — stick X={}  Y={}  throttle={}",
                        axis_raw(st, "X"),
                        axis_raw(st, "Y"),
                        axis_raw(st, "Z")
                    ))
                    .size(13.0)
                    .color(OK),
                );
            } else {
                ui.label(
                    RichText::new("Waiting for live input…")
                        .size(13.0)
                        .color(MUTED),
                );
            }
            ui.add_space(16.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("Continue").color(Color32::WHITE).strong(),
                        )
                        .fill(ACCENT)
                        .min_size(Vec2::new(96.0, 28.0)),
                    )
                    .clicked()
                {
                    app.finish_first_run();
                }
            });
        });
}

fn draw_confirm(
    ctx: &egui::Context,
    kind: Confirm,
    slot: &mut Option<Confirm>,
    mut on_ok: impl FnMut(Confirm),
) {
    let (title, body, action) = match kind {
        Confirm::InstallApp => (
            "Install",
            "Copy X52 Integrity into your user profile and add Start Menu and Desktop shortcuts.",
            "Install",
        ),
        Confirm::RemoveApp => (
            "Uninstall",
            "Remove shortcuts and the local install folder. Device drivers are not changed.",
            "Uninstall",
        ),
        Confirm::InboxHid => (
            "Windows HID",
            "Bind this X52 to the Windows HID driver. Windows will ask for administrator approval.",
            "Continue",
        ),
        Confirm::RestoreLogitech => (
            "Logitech driver",
            "Restore the saved Logitech X52 package. Windows will ask for administrator approval.",
            "Continue",
        ),
    };
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .frame(Frame {
            inner_margin: Margin::same(18),
            corner_radius: CornerRadius::same(8),
            fill: CARD,
            stroke: Stroke::new(1.0_f32, STROKE),
            ..Default::default()
        })
        .show(ctx, |ui| {
            ui.set_max_width(440.0);
            ui.label(RichText::new(body).size(14.0).color(TEXT));
            ui.add_space(16.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(RichText::new(action).color(Color32::WHITE).strong())
                            .fill(ACCENT)
                            .min_size(Vec2::new(96.0, 28.0)),
                    )
                    .clicked()
                {
                    on_ok(kind);
                }
                if ui
                    .add(egui::Button::new("Cancel").min_size(Vec2::new(80.0, 28.0)))
                    .clicked()
                {
                    *slot = None;
                }
            });
        });
}
