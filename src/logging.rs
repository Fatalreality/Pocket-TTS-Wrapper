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

/// Removes a Hugging Face token from a message before it is buffered, printed or emitted.
///
/// Two things are caught: the exact token this process holds, and anything shaped like one
/// (`hf_` followed by a long run of token characters), so a token cannot reach the in-app
/// console, a saved log or stderr even if some future code logs a request or a header.
pub fn redact(message: &str) -> String {
    let mut text = message.to_string();
    if let Ok(token) = std::env::var("HF_TOKEN") {
        let token = token.trim();
        if token.len() >= 8 {
            text = text.replace(token, "hf_***");
        }
    }
    redact_token_shapes(&text)
}

fn redact_token_shapes(message: &str) -> String {
    /// `hf_` plus this many token characters is a credential, not a word like `hf_hub`.
    const MIN_TOKEN_BODY: usize = 20;
    const PREFIX: &str = "hf_";

    let mut out = String::with_capacity(message.len());
    let mut rest = message;
    while let Some(index) = rest.find(PREFIX) {
        let (before, after) = rest.split_at(index + PREFIX.len());
        out.push_str(before);
        let body = after
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
            .unwrap_or(after.len());
        if body >= MIN_TOKEN_BODY {
            out.push_str("***");
        } else {
            out.push_str(&after[..body]);
        }
        rest = &after[body..];
    }
    out.push_str(rest);
    out
}

pub fn add_log_entry(level: &str, message: &str) {
    let message = redact(message);
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

#[cfg(test)]
mod tests {
    use super::redact;

    #[test]
    fn tokens_are_removed_from_messages() {
        let token = "hf_0123456789abcdef0123456789abcdef0123";
        assert_eq!(redact(&format!("auth {token} here")), "auth hf_*** here");
        // The same text again, as it would arrive from the environment check.
        assert_eq!(redact("auth hf_*** here"), "auth hf_*** here");
    }

    #[test]
    fn ordinary_words_are_left_alone() {
        assert_eq!(redact("reading hf_hub cache"), "reading hf_hub cache");
        assert_eq!(redact("token=hf_token"), "token=hf_token");
        assert_eq!(redact("no prefix here"), "no prefix here");
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
        let msg = redact(&format!("{}", record.args()));
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
