use std::path::PathBuf;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

static GUARD: std::sync::OnceLock<WorkerGuard> = std::sync::OnceLock::new();

const MAX_LOG_FILES: usize = 7;

/// The newest log file's last `limit` lines, for the diagnostics bundle.
pub fn tail(limit: usize) -> String {
    let dir = log_dir();
    let newest = std::fs::read_dir(&dir).ok().and_then(|entries| {
        entries
            .flatten()
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "log"))
            .filter_map(|entry| {
                let stamp = entry.metadata().ok()?.modified().ok()?;
                Some((stamp, entry.path()))
            })
            .max_by_key(|(stamp, _)| *stamp)
            .map(|(_, path)| path)
    });
    let Some(path) = newest else {
        return "(no logs yet)".to_string();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return "(log unreadable)".to_string();
    };
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(limit)..].join("\n")
}

fn log_dir() -> PathBuf {
    #[cfg(windows)]
    {
        crate::config::paths::logs_dir()
    }
    #[cfg(not(windows))]
    {
        std::env::temp_dir().join("slate-logs")
    }
}

pub fn init() {
    let dir = log_dir();
    let _ = std::fs::create_dir_all(&dir);
    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("slate")
        .filename_suffix("log")
        .max_log_files(MAX_LOG_FILES)
        .build(&dir)
        .unwrap_or_else(|_| tracing_appender::rolling::never(&dir, "slate.log"));
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let _ = GUARD.set(guard);

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let fmt_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true);

    let _ = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .try_init();
}
