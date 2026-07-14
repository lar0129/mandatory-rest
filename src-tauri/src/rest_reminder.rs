use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewUrl,
    WebviewWindowBuilder,
};

use crate::settings::Settings;
use crate::tray::{self, TrayState};

const WINDOW_PREFIX: &str = "rest-reminder-";
const SCHEDULER_TICK: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, PartialEq)]
struct ReminderConfig {
    enabled: bool,
    interval_secs: u32,
    duration_secs: u32,
    message: String,
    allow_skip: bool,
    pause_warning_secs: u32,
}

impl From<&Settings> for ReminderConfig {
    fn from(settings: &Settings) -> Self {
        Self {
            enabled: settings.rest_reminder_enabled,
            interval_secs: settings.rest_reminder_interval_secs.max(1),
            duration_secs: settings.rest_reminder_duration_secs.max(1),
            message: settings.rest_reminder_message.clone(),
            allow_skip: settings.rest_reminder_allow_skip,
            pause_warning_secs: settings.rest_reminder_pause_warning_secs.max(60),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PauseReason {
    Manual,
    Pomodoro,
}

impl PauseReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Pomodoro => "pomodoro",
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

#[derive(Debug)]
struct SchedulerRuntime {
    config: ReminderConfig,
    pomodoro_active: bool,
    active: Option<ActiveReminder>,
    next_due: Option<Instant>,
    paused_remaining_secs: u32,
    pause_reason: Option<PauseReason>,
    manual_pause_started: Option<Instant>,
    next_warning: Option<Instant>,
}

impl SchedulerRuntime {
    fn new(config: ReminderConfig) -> Self {
        let next_due = config
            .enabled
            .then(|| Instant::now() + Duration::from_secs(config.interval_secs as u64));
        Self {
            paused_remaining_secs: config.interval_secs,
            config,
            pomodoro_active: false,
            active: None,
            next_due,
            pause_reason: None,
            manual_pause_started: None,
            next_warning: None,
        }
    }

    fn snapshot(&self) -> RestReminderSnapshot {
        let (active, message, overlay_remaining_secs, allow_skip, reason) = match &self.active {
            Some(active) => (
                true,
                active.message.clone(),
                remaining_secs(active.deadline),
                active.allow_skip,
                active.reason.as_str().to_string(),
            ),
            None => (false, String::new(), 0, false, String::new()),
        };

        let timer_remaining_secs = if !self.config.enabled {
            0
        } else if let Some(deadline) = self.next_due {
            remaining_secs(deadline)
        } else {
            self.paused_remaining_secs.min(self.config.interval_secs)
        };

        RestReminderSnapshot {
            active,
            message,
            remaining_secs: overlay_remaining_secs,
            allow_skip,
            reason,
            enabled: self.config.enabled,
            timer_remaining_secs,
            timer_total_secs: self.config.interval_secs,
            is_paused: self.pause_reason.is_some(),
            pause_reason: self
                .pause_reason
                .map(PauseReason::as_str)
                .unwrap_or_default()
                .to_string(),
            next_pause_warning_secs: self.next_warning.map(remaining_secs).unwrap_or(0),
        }
    }

    fn reset_schedule(&mut self) {
        self.paused_remaining_secs = self.config.interval_secs;
        self.manual_pause_started = None;
        self.next_warning = None;

        if !self.config.enabled {
            self.next_due = None;
            self.pause_reason = None;
        } else if self.pomodoro_active {
            self.next_due = None;
            self.pause_reason = Some(PauseReason::Pomodoro);
        } else if self.active.is_some() {
            self.next_due = None;
            self.pause_reason = None;
        } else {
            self.next_due =
                Some(Instant::now() + Duration::from_secs(self.config.interval_secs as u64));
            self.pause_reason = None;
        }
    }

    fn pomodoro_started(&mut self) {
        self.pomodoro_active = true;
        self.paused_remaining_secs = self
            .next_due
            .map(remaining_secs)
            .unwrap_or(self.paused_remaining_secs)
            .max(1);
        self.next_due = None;
        self.pause_reason = Some(PauseReason::Pomodoro);
        self.manual_pause_started = None;
        self.next_warning = None;
    }

    fn pomodoro_stopped(&mut self) {
        self.pomodoro_active = false;
        self.reset_schedule();
    }

    fn manual_pause(&mut self) {
        if !self.config.enabled || self.pomodoro_active || self.active.is_some() {
            return;
        }

        self.paused_remaining_secs = self
            .next_due
            .map(remaining_secs)
            .unwrap_or(self.paused_remaining_secs)
            .max(1);
        self.next_due = None;
        self.pause_reason = Some(PauseReason::Manual);
        let now = Instant::now();
        self.manual_pause_started = Some(now);
        self.next_warning =
            Some(now + Duration::from_secs(self.config.pause_warning_secs.max(60) as u64));
    }

    fn manual_resume(&mut self) {
        if self.pause_reason != Some(PauseReason::Manual)
            || self.pomodoro_active
            || !self.config.enabled
        {
            return;
        }

        self.next_due =
            Some(Instant::now() + Duration::from_secs(self.paused_remaining_secs.max(1) as u64));
        self.pause_reason = None;
        self.manual_pause_started = None;
        self.next_warning = None;
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RestReminderSnapshot {
    /// Whether the full-screen black overlay is currently visible.
    pub active: bool,
    pub message: String,
    /// Remaining seconds for the active black overlay.
    pub remaining_secs: u32,
    pub allow_skip: bool,
    pub reason: String,
    /// State of the independent background forced-rest countdown.
    pub enabled: bool,
    pub timer_remaining_secs: u32,
    pub timer_total_secs: u32,
    pub is_paused: bool,
    /// "manual" | "pomodoro" | ""
    pub pause_reason: String,
    /// Seconds until the next repeated warning for a manual pause.
    pub next_pause_warning_secs: u32,
}

impl RestReminderSnapshot {
    fn initial(config: &ReminderConfig) -> Self {
        Self {
            active: false,
            message: String::new(),
            remaining_secs: 0,
            allow_skip: false,
            reason: String::new(),
            enabled: config.enabled,
            timer_remaining_secs: if config.enabled {
                config.interval_secs
            } else {
                0
            },
            timer_total_secs: config.interval_secs,
            is_paused: false,
            pause_reason: String::new(),
            next_pause_warning_secs: 0,
        }
    }
}

enum ControlMessage {
    ApplySettings(ReminderConfig),
    PomodoroStarted,
    PomodoroStopped,
    PomodoroWorkCompleted,
    Pause,
    Resume,
    Reset,
    Preview,
    Dismiss,
}

/// Background scheduler for periodic rest reminders.
///
/// It is independent from the WebView so countdowns, tray labels, overlays,
/// and manual-pause warnings continue to work while the main window is hidden.
pub struct RestReminderController {
    tx: mpsc::Sender<ControlMessage>,
    shared: Arc<Mutex<RestReminderSnapshot>>,
}

impl RestReminderController {
    pub fn new(app: AppHandle, settings: &Settings, tray: Arc<TrayState>) -> Self {
        let (tx, rx) = mpsc::channel();
        let initial_config = ReminderConfig::from(settings);
        let shared = Arc::new(Mutex::new(RestReminderSnapshot::initial(&initial_config)));
        let shared_thread = Arc::clone(&shared);

        std::thread::Builder::new()
            .name("rest-reminder".to_string())
            .spawn(move || run_scheduler(app, rx, shared_thread, tray, initial_config))
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

    pub fn pause(&self) {
        let _ = self.tx.send(ControlMessage::Pause);
    }

    pub fn resume(&self) {
        let _ = self.tx.send(ControlMessage::Resume);
    }

    pub fn reset(&self) {
        let _ = self.tx.send(ControlMessage::Reset);
    }

    pub fn preview(&self) {
        let _ = self.tx.send(ControlMessage::Preview);
    }

    pub fn dismiss(&self) {
        let _ = self.tx.send(ControlMessage::Dismiss);
    }

    pub fn snapshot(&self) -> RestReminderSnapshot {
        self.shared.lock().unwrap().clone()
    }
}

fn run_scheduler(
    app: AppHandle,
    rx: mpsc::Receiver<ControlMessage>,
    shared: Arc<Mutex<RestReminderSnapshot>>,
    tray: Arc<TrayState>,
    initial_config: ReminderConfig,
) {
    let mut runtime = SchedulerRuntime::new(initial_config);
    let mut last_published: Option<RestReminderSnapshot> = None;

    loop {
        match rx.recv_timeout(SCHEDULER_TICK) {
            Ok(ControlMessage::ApplySettings(updated)) => {
                apply_settings(&app, &mut runtime, updated);
            }
            Ok(ControlMessage::PomodoroStarted) => {
                if !runtime.pomodoro_active {
                    log::info!("[rest] background countdown paused by Pomodoro");
                }
                runtime.pomodoro_started();
            }
            Ok(ControlMessage::PomodoroStopped) => {
                if runtime.pomodoro_active {
                    log::info!("[rest] Pomodoro inactive; background countdown reset");
                }
                runtime.pomodoro_stopped();
            }
            Ok(ControlMessage::PomodoroWorkCompleted) => {
                runtime.pomodoro_active = false;
                runtime.pause_reason = None;
                runtime.next_due = None;
                runtime.paused_remaining_secs = runtime.config.interval_secs;
                runtime.manual_pause_started = None;
                runtime.next_warning = None;
                if runtime.config.enabled {
                    log::info!("[rest] showing overlay after completed Pomodoro work round");
                    let duration_secs = runtime.config.duration_secs;
                    let allow_skip = runtime.config.allow_skip;
                    activate(
                        &app,
                        &mut runtime,
                        ReminderReason::Pomodoro,
                        duration_secs,
                        allow_skip,
                    );
                } else {
                    runtime.reset_schedule();
                }
            }
            Ok(ControlMessage::Pause) => {
                if runtime.pause_reason != Some(PauseReason::Manual) {
                    log::info!("[rest] background countdown manually paused");
                }
                runtime.manual_pause();
            }
            Ok(ControlMessage::Resume) => {
                if runtime.pause_reason == Some(PauseReason::Manual) {
                    log::info!("[rest] background countdown manually resumed");
                }
                runtime.manual_resume();
            }
            Ok(ControlMessage::Reset) => {
                log::info!("[rest] background countdown reset");
                runtime.reset_schedule();
            }
            Ok(ControlMessage::Preview) => {
                if runtime.active.is_none() {
                    let preview_secs = runtime.config.duration_secs.clamp(5, 10);
                    activate(
                        &app,
                        &mut runtime,
                        ReminderReason::Preview,
                        preview_secs,
                        true,
                    );
                }
            }
            Ok(ControlMessage::Dismiss) => {
                let can_dismiss = runtime
                    .active
                    .as_ref()
                    .map(|active| active.allow_skip)
                    .unwrap_or(false);
                if can_dismiss {
                    log::info!("[rest] overlay skipped by user");
                    finish_active(&app, &mut runtime);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        let now = Instant::now();
        let expired = runtime
            .active
            .as_ref()
            .map(|active| now >= active.deadline)
            .unwrap_or(false);
        if expired {
            log::info!("[rest] overlay completed");
            finish_active(&app, &mut runtime);
        }

        if runtime.active.is_none()
            && runtime.config.enabled
            && !runtime.pomodoro_active
            && runtime.pause_reason.is_none()
            && runtime
                .next_due
                .map(|deadline| now >= deadline)
                .unwrap_or(false)
        {
            log::info!("[rest] showing scheduled overlay");
            runtime.next_due = None;
            let duration_secs = runtime.config.duration_secs;
            let allow_skip = runtime.config.allow_skip;
            activate(
                &app,
                &mut runtime,
                ReminderReason::Scheduled,
                duration_secs,
                allow_skip,
            );
        }

        if runtime.active.is_none()
            && runtime.pause_reason == Some(PauseReason::Manual)
            && !runtime.pomodoro_active
            && runtime
                .next_warning
                .map(|deadline| now >= deadline)
                .unwrap_or(false)
        {
            let paused_secs = runtime
                .manual_pause_started
                .map(|started| now.saturating_duration_since(started).as_secs() as u32)
                .unwrap_or(runtime.config.pause_warning_secs);
            log::info!("[rest] manual pause warning elapsed={paused_secs}s");
            let _ = app.emit(
                "rest:pause-warning",
                serde_json::json!({ "paused_secs": paused_secs }),
            );
            runtime.next_warning =
                Some(now + Duration::from_secs(runtime.config.pause_warning_secs.max(60) as u64));
        }

        publish_state(&app, &tray, &shared, &runtime, &mut last_published);
    }
}

fn apply_settings(app: &AppHandle, runtime: &mut SchedulerRuntime, updated: ReminderConfig) {
    if updated == runtime.config {
        return;
    }

    let old = runtime.config.clone();
    log::info!(
        "[rest] settings updated enabled={} interval={}s duration={}s skip={} pause_warning={}s",
        updated.enabled,
        updated.interval_secs,
        updated.duration_secs,
        updated.allow_skip,
        updated.pause_warning_secs
    );
    runtime.config = updated;

    if !runtime.config.enabled {
        clear_active(app, runtime);
        runtime.reset_schedule();
        return;
    }

    if !old.enabled || old.interval_secs != runtime.config.interval_secs {
        runtime.reset_schedule();
    } else if old.pause_warning_secs != runtime.config.pause_warning_secs
        && runtime.pause_reason == Some(PauseReason::Manual)
    {
        runtime.next_warning = Some(
            Instant::now() + Duration::from_secs(runtime.config.pause_warning_secs.max(60) as u64),
        );
    }
}

fn publish_state(
    app: &AppHandle,
    tray_state: &Arc<TrayState>,
    shared: &Arc<Mutex<RestReminderSnapshot>>,
    runtime: &SchedulerRuntime,
    last_published: &mut Option<RestReminderSnapshot>,
) {
    let snapshot = runtime.snapshot();
    *shared.lock().unwrap() = snapshot.clone();

    if last_published.as_ref() == Some(&snapshot) {
        return;
    }

    tray::update_rest_time(tray_state, &snapshot);
    let _ = app.emit("rest:state", &snapshot);
    *last_published = Some(snapshot);
}

fn activate(
    app: &AppHandle,
    runtime: &mut SchedulerRuntime,
    reason: ReminderReason,
    duration_secs: u32,
    allow_skip: bool,
) {
    runtime.active = Some(ActiveReminder {
        deadline: Instant::now() + Duration::from_secs(duration_secs.max(1) as u64),
        message: runtime.config.message.clone(),
        allow_skip,
        reason,
    });
    show_overlay_windows(app);
}

fn finish_active(app: &AppHandle, runtime: &mut SchedulerRuntime) {
    let reason = runtime.active.as_ref().map(|active| active.reason);
    clear_active(app, runtime);

    match reason {
        Some(ReminderReason::Scheduled) | Some(ReminderReason::Pomodoro) => {
            runtime.reset_schedule();
        }
        Some(ReminderReason::Preview) => {
            if runtime
                .next_due
                .map(|deadline| Instant::now() >= deadline)
                .unwrap_or(false)
            {
                runtime.next_due = Some(
                    Instant::now()
                        + Duration::from_secs(runtime.config.interval_secs.max(1) as u64),
                );
            }
        }
        None => {}
    }
}

fn clear_active(app: &AppHandle, runtime: &mut SchedulerRuntime) {
    if runtime.active.take().is_some() {
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
            .title("Pomotroid - Rest")
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
                    .title("Pomotroid - Rest")
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

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> ReminderConfig {
        ReminderConfig {
            enabled: true,
            interval_secs: 1200,
            duration_secs: 20,
            message: "Rest".to_string(),
            allow_skip: true,
            pause_warning_secs: 1200,
        }
    }

    #[test]
    fn manual_pause_and_resume_preserve_remaining_time() {
        let mut runtime = SchedulerRuntime::new(config());
        runtime.next_due = Some(Instant::now() + Duration::from_secs(600));
        runtime.manual_pause();
        assert_eq!(runtime.pause_reason, Some(PauseReason::Manual));
        assert!((599..=600).contains(&runtime.paused_remaining_secs));
        assert!(runtime.next_warning.is_some());

        runtime.manual_resume();
        assert_eq!(runtime.pause_reason, None);
        assert!(runtime.next_due.is_some());
        assert!(runtime.next_warning.is_none());
    }

    #[test]
    fn stopping_pomodoro_resets_background_countdown() {
        let mut runtime = SchedulerRuntime::new(config());
        runtime.next_due = Some(Instant::now() + Duration::from_secs(60));
        runtime.pomodoro_started();
        assert_eq!(runtime.pause_reason, Some(PauseReason::Pomodoro));
        assert!(runtime.next_due.is_none());

        runtime.pomodoro_stopped();
        let snapshot = runtime.snapshot();
        assert!(!snapshot.is_paused);
        assert!((1199..=1200).contains(&snapshot.timer_remaining_secs));
    }

    #[test]
    fn pomodoro_start_cancels_manual_pause_warning() {
        let mut runtime = SchedulerRuntime::new(config());
        runtime.manual_pause();
        assert!(runtime.next_warning.is_some());
        runtime.pomodoro_started();
        assert_eq!(runtime.pause_reason, Some(PauseReason::Pomodoro));
        assert!(runtime.next_warning.is_none());
        assert!(runtime.manual_pause_started.is_none());
    }
}
