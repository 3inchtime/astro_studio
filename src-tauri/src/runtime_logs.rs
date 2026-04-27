use crate::models::RuntimeLogEntry;
use chrono::{SecondsFormat, Utc};
use log::{LevelFilter, Log, Metadata, Record};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter};

const RUNTIME_LOG_BUFFER_LIMIT: usize = 500;

static APP_HANDLE: OnceLock<Mutex<Option<AppHandle>>> = OnceLock::new();
static LOG_BUFFER: OnceLock<Mutex<VecDeque<RuntimeLogEntry>>> = OnceLock::new();
static LOG_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn app_handle() -> &'static Mutex<Option<AppHandle>> {
    APP_HANDLE.get_or_init(|| Mutex::new(None))
}

fn log_buffer() -> &'static Mutex<VecDeque<RuntimeLogEntry>> {
    LOG_BUFFER.get_or_init(|| Mutex::new(VecDeque::with_capacity(RUNTIME_LOG_BUFFER_LIMIT)))
}

struct RuntimeLogger {
    inner: env_logger::Logger,
}

impl Log for RuntimeLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        self.inner.enabled(metadata)
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        self.inner.log(record);

        let entry = RuntimeLogEntry {
            sequence: LOG_SEQUENCE.fetch_add(1, Ordering::Relaxed),
            timestamp: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            level: record.level().to_string().to_lowercase(),
            target: record.target().to_string(),
            message: record.args().to_string(),
        };

        {
            let mut buffer = log_buffer().lock().expect("runtime log buffer poisoned");
            if buffer.len() >= RUNTIME_LOG_BUFFER_LIMIT {
                buffer.pop_front();
            }
            buffer.push_back(entry.clone());
        }

        let handle = app_handle()
            .lock()
            .expect("runtime log app handle poisoned")
            .clone();
        if let Some(app) = handle {
            let _ = app.emit("runtime-log:new", &entry);
        }
    }

    fn flush(&self) {
        self.inner.flush();
    }
}

pub fn init_logger(level: LevelFilter) {
    let mut builder = env_logger::Builder::new();
    builder.format_timestamp_millis().filter_level(level);
    let inner = builder.build();

    log::set_max_level(level);
    log::set_boxed_logger(Box::new(RuntimeLogger { inner }))
        .expect("runtime logger should only be initialized once");
}

pub fn attach_app_handle(handle: AppHandle) {
    let mut slot = app_handle()
        .lock()
        .expect("runtime log app handle poisoned");
    *slot = Some(handle);
}

pub fn recent_logs(limit: usize) -> Vec<RuntimeLogEntry> {
    let limit = limit.min(RUNTIME_LOG_BUFFER_LIMIT);
    let buffer = log_buffer().lock().expect("runtime log buffer poisoned");
    let skip = buffer.len().saturating_sub(limit);
    buffer.iter().skip(skip).cloned().collect()
}
