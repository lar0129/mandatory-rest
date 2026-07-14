use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewUrl,
    WebviewWindowBuilder,
};

use crate::settings::Settings;

const WINDOW_PREFIX: &str = "rest-reminder-";
const SCHEDULER_TICK: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, PartialEq)]
struct ReminderConfig {
    enabled: bool,
    interval_secs: u32,
    duration_secs: u32,
    message: String,
    allow_skip: bool,
}

impl From<&Settings> for ReminderConfig {
    fn from(settings: &Settings) -> Self {
        Self {
            enabled: settings.rest_reminder_enabled,
            interval_secs: settings.rest_reminder_interval_secs.max(1),
            duration_secs: settings.rest_reminder_duration_secs.max(1),
            message: settings.rest_reminder_message.clone(),
            allow_skip: settings.rest_reminder_allow_skip,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReminderReason {
    Scheduled,
    Pomodoro,
    Preview,
}

impl ReminderReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::Scheduled => "scheduled",
            Self::Pomodoro => "pomodoro",
            Self::Preview => "preview",
        }
    }
}

#[derive(Debug, Clone)]
struct ActiveReminder {
    deadline: Instant,
    message: String,
    allow_skip: bool,
    reason: ReminderReason,
}

#[derive(Debug, Default)]
struct SharedState {
    active: Option<ActiveReminder>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RestReminderSnapshot {
    pub active: bool,
    pub message: String,
    pub remaining_secs: u32,
    pub allow_skip: bool,
    pub reason: String,
}

enum ControlMessage {
    ApplySettings(ReminderConfig),
    PomodoroStarted,
    PomodoroStopped,
    PomodoroWorkCompleted,
    Preview,
    Dismiss,
}

/// Background scheduler for periodic rest reminders.
///
/// The scheduler is deliberately independent from the WebView so it remains
/// accurate while Pomotroid is hidden in the system tray. Starting a Pomodoro
/// session suspends the periodic deadline; a naturally completed work round
/// opens the same overlay immediately instead.
pub struct RestReminderController {
    tx: mpsc::Sender<ControlMessage>,
    shared: Arc<Mutex<SharedState>>,
}

impl RestReminderController {
    pub fn new(app: AppHandle, settings: &Settings) -> Self {
        let (tx, rx) = mpsc::channel();
        let shared = Arc::new(Mutex::new(SharedState::default()));
        let shared_thread = Arc::clone(&shared);
        let initial_config = ReminderConfig::from(settings);

        std::thread::Builder::new()
            .name("rest-reminder".to_string())
            .spawn(move || run_scheduler(app, rx, shared_thread, initial_config))
            .expect("failed to spawn rest reminder scheduler");

        Self { tx, shared }
    }

    pub fn apply_settings(&self, settings: &Settings) {
        let _ = self
            .tx
            .send(ControlMessage::ApplySettings(ReminderConfig::from(
                settings,
            )));
    }

    pub fn pomodoro_started(&self) {
        let _ = self.tx.send(ControlMessage::PomodoroStarted);
    }

    pub fn pomodoro_stopped(&self) {
        let _ = self.tx.send(ControlMessage::PomodoroStopped);
    }

    pub fn pomodoro_work_completed(&self) {
        let _ = self.tx.send(ControlMessage::PomodoroWorkCompleted);
    }

    pub fn preview(&self) {
        let _ = self.tx.send(ControlMessage::Preview);
    }

    pub fn dismiss(&self) {
        let _ = self.tx.send(ControlMessage::Dismiss);
    }

    pub fn snapshot(&self) -> RestReminderSnapshot {
        let state = self.shared.lock().unwrap();
        match &state.active {
            Some(active) => RestReminderSnapshot {
                active: true,
                message: active.message.clone(),
                remaining_secs: remaining_secs(active.deadline),
                allow_skip: active.allow_skip,
                reason: active.reason.as_str().to_string(),
            },
            None => RestReminderSnapshot {
                active: false,
                message: String::new(),
                remaining_secs: 0,
                allow_skip: false,
                reason: String::new(),
            },
        }
    }
}

fn run_scheduler(
    app: AppHandle,
    rx: mpsc::Receiver<ControlMessage>,
    shared: Arc<Mutex<SharedState>>,
    mut config: ReminderConfig,
) {
    let mut pomodoro_active = false;
    let mut next_due = scheduled_deadline(&config, pomodoro_active);

    loop {
        match rx.recv_timeout(SCHEDULER_TICK) {
            Ok(ControlMessage::ApplySettings(updated)) => {
                if updated != config {
                    log::info!(
                        "[rest] settings updated enabled={} interval={}s duration={}s skip={}",
                        updated.enabled,
                        updated.interval_secs,
                        updated.duration_secs,
                        updated.allow_skip
                    );
                    config = updated;
                    clear_active(&app, &shared);
                    next_due = scheduled_deadline(&config, pomodoro_active);
                }
            }
            Ok(ControlMessage::PomodoroStarted) => {
                if !pomodoro_active {
                    log::info!("[rest] periodic reminder suspended by Pomodoro");
                }
                pomodoro_active = true;
                next_due = None;
                clear_active(&app, &shared);
            }
            Ok(ControlMessage::PomodoroStopped) => {
                if pomodoro_active {
                    log::info!("[rest] periodic reminder resumed after Pomodoro reset");
                }
                pomodoro_active = false;
                clear_active(&app, &shared);
                next_due = scheduled_deadline(&config, pomodoro_active);
            }
            Ok(ControlMessage::PomodoroWorkCompleted) => {
                if config.enabled {
                    log::info!("[rest] showing overlay after completed Pomodoro work round");
                    activate(
                        &app,
                        &shared,
                        &config,
                        ReminderReason::Pomodoro,
                        config.duration_secs,
                        config.allow_skip,
                    );
                }
                next_due = None;
            }
            Ok(ControlMessage::Preview) => {
                let preview_secs = config.duration_secs.clamp(5, 10);
                activate(
                    &app,
                    &shared,
                    &config,
                    ReminderReason::Preview,
                    preview_secs,
                    true,
                );
            }
            Ok(ControlMessage::Dismiss) => {
                let can_dismiss = shared
                    .lock()
                    .unwrap()
                    .active
                    .as_ref()
                    .map(|active| active.allow_skip)
                    .unwrap_or(false);
                if can_dismiss {
                    log::info!("[rest] overlay skipped by user");
                    clear_active(&app, &shared);
                    if !pomodoro_active {
                        next_due = scheduled_deadline(&config, false);
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        let expired = shared
            .lock()
            .unwrap()
            .active
            .as_ref()
            .map(|active| Instant::now() >= active.deadline)
            .unwrap_or(false);
        if expired {
            log::info!("[rest] overlay completed");
            clear_active(&app, &shared);
            if !pomodoro_active {
                next_due = scheduled_deadline(&config, false);
            }
        }

        let overlay_active = shared.lock().unwrap().active.is_some();
        if !overlay_active
            && config.enabled
            && !pomodoro_active
            && next_due
                .map(|deadline| Instant::now() >= deadline)
                .unwrap_or(false)
        {
            log::info!("[rest] showing scheduled overlay");
            activate(
                &app,
                &shared,
                &config,
                ReminderReason::Scheduled,
                config.duration_secs,
                config.allow_skip,
            );
            next_due = None;
        }
    }
}

fn scheduled_deadline(config: &ReminderConfig, pomodoro_active: bool) -> Option<Instant> {
    (config.enabled && !pomodoro_active)
        .then(|| Instant::now() + Duration::from_secs(config.interval_secs as u64))
}

fn activate(
    app: &AppHandle,
    shared: &Arc<Mutex<SharedState>>,
    config: &ReminderConfig,
    reason: ReminderReason,
    duration_secs: u32,
    allow_skip: bool,
) {
    let active = ActiveReminder {
        deadline: Instant::now() + Duration::from_secs(duration_secs.max(1) as u64),
        message: config.message.clone(),
        allow_skip,
        reason,
    };
    shared.lock().unwrap().active = Some(active);
    show_overlay_windows(app);
}

fn clear_active(app: &AppHandle, shared: &Arc<Mutex<SharedState>>) {
    let had_active = shared.lock().unwrap().active.take().is_some();
    if had_active {
        close_overlay_windows(app);
    }
}

fn remaining_secs(deadline: Instant) -> u32 {
    let remaining = deadline.saturating_duration_since(Instant::now());
    remaining.as_secs_f64().ceil() as u32
}

fn close_existing_windows(app: &AppHandle) {
    for (label, window) in app.webview_windows() {
        if label.starts_with(WINDOW_PREFIX) {
            let _ = window.close();
        }
    }
}

fn close_overlay_windows(app: &AppHandle) {
    let app_handle = app.clone();
    if let Err(error) = app.run_on_main_thread(move || close_existing_windows(&app_handle)) {
        log::error!("[rest] failed to queue overlay close: {error}");
    }
}

fn show_overlay_windows(app: &AppHandle) {
    let app_handle = app.clone();
    if let Err(error) = app.run_on_main_thread(move || {
        close_existing_windows(&app_handle);

        let monitors = app_handle
            .get_webview_window("main")
            .and_then(|window| window.available_monitors().ok())
            .unwrap_or_default();

        if monitors.is_empty() {
            match WebviewWindowBuilder::new(
                &app_handle,
                format!("{WINDOW_PREFIX}0"),
                WebviewUrl::App("rest".into()),
            )
            .title("Pomotroid — Rest")
            .decorations(false)
            .resizable(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .fullscreen(true)
            .background_color(tauri::webview::Color(0, 0, 0, 255))
            .build()
            {
                Ok(_) => {}
                Err(error) => log::error!("[rest] failed to create fallback overlay: {error}"),
            }
            return;
        }

        for (index, monitor) in monitors.iter().enumerate() {
            let label = format!("{WINDOW_PREFIX}{index}");
            let builder =
                WebviewWindowBuilder::new(&app_handle, label, WebviewUrl::App("rest".into()))
                    .title("Pomotroid — Rest")
                    .decorations(false)
                    .resizable(false)
                    .always_on_top(true)
                    .skip_taskbar(true)
                    .visible(false)
                    .background_color(tauri::webview::Color(0, 0, 0, 255));

            match builder.build() {
                Ok(window) => {
                    let position = monitor.position();
                    let size = monitor.size();
                    let _ = window.set_position(Position::Physical(PhysicalPosition::new(
                        position.x, position.y,
                    )));
                    let _ =
                        window.set_size(Size::Physical(PhysicalSize::new(size.width, size.height)));
                }
                Err(error) => {
                    log::error!("[rest] failed to create overlay for monitor {index}: {error}");
                }
            }
        }
    }) {
        log::error!("[rest] failed to queue overlay creation: {error}");
    }
}
