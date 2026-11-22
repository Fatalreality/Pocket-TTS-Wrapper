//! Logs as the Python package prints them: `LEVEL: message` on stderr, for this crate only,
//! and buffers them for the integrated in-app terminal and Tauri frontend events.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;
use tauri::Emitter;

const MAX_LOG_ENTRIES: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

static LOG_BUFFER: OnceLock<Mutex<VecDeque<LogEntry>>> = OnceLock::new();
static APP_HANDLE: OnceLock<Mutex<Option<tauri::AppHandle>>> = OnceLock::new();

fn get_buffer() -> &'static Mutex<VecDeque<LogEntry>> {
    LOG_BUFFER.get_or_init(|| Mutex::new(VecDeque::with_capacity(MAX_LOG_ENTRIES)))
}

fn get_app_handle() -> &'static Mutex<Option<tauri::AppHandle>> {
    APP_HANDLE.get_or_init(|| Mutex::new(None))
}

pub fn set_app_handle(handle: tauri::AppHandle) {
    if let Ok(mut lock) = get_app_handle().lock() {
        *lock = Some(handle);
    }
}

pub fn add_log_entry(level: &str, message: &str) {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let hours = (now / 3600) % 24;
    let mins = (now / 60) % 60;
    let secs = now % 60;
    let timestamp = format!("{hours:02}:{mins:02}:{secs:02}");

    let entry = LogEntry {
        timestamp,
        level: level.to_string(),
        message: message.to_string(),
    };

    if let Ok(mut buf) = get_buffer().lock() {
        if buf.len() >= MAX_LOG_ENTRIES {
            buf.pop_front();
        }
        buf.push_back(entry.clone());
    }

    if let Ok(guard) = get_app_handle().lock() {
        if let Some(app) = guard.as_ref() {
            let _ = app.emit("terminal_log", &entry);
        }
    }
}

pub fn get_recent_logs() -> Vec<LogEntry> {
    if let Ok(buf) = get_buffer().lock() {
        buf.iter().cloned().collect()
    } else {
        Vec::new()
    }
}

pub fn clear_logs() {
    if let Ok(mut buf) = get_buffer().lock() {
        buf.clear();
    }
}

struct Logger {
    level: log::LevelFilter,
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.level && (metadata.target().starts_with("pocket_tts") || metadata.target().starts_with("pocket_tts_app"))
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let level = match record.level() {
            log::Level::Error => "ERROR",
            log::Level::Warn => "WARN",
            log::Level::Info => "INFO",
            log::Level::Debug => "DEBUG",
            log::Level::Trace => "TRACE",
        };
        let msg = format!("{}", record.args());
        add_log_entry(level, &msg);
        eprintln!("{level}: {msg}");
    }

    fn flush(&self) {}
}

/// `quiet` keeps the errors only.
pub fn init(quiet: bool) {
    let level = if quiet {
        log::LevelFilter::Error
    } else {
        log::LevelFilter::Info
    };
    let _ = log::set_boxed_logger(Box::new(Logger { level }));
    log::set_max_level(level);
}
