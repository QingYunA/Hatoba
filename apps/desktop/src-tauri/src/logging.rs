//! Daily-rotated local logs kept for 7 days (spec §11). Never log secrets or terminal content (SEC-04).

use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub fn init(log_dir: &Path) -> Option<WorkerGuard> {
    let filter = EnvFilter::try_from_env("HATOBA_LOG").unwrap_or_else(|_| {
        EnvFilter::new("info,russh=warn,russh_sftp=warn,tao=warn,wry=warn,reqwest=warn,hyper=warn")
    });
    let file = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("hatoba")
        .filename_suffix("log")
        .max_log_files(7)
        .build(log_dir)
        .ok();
    let (writer, guard) = match file {
        Some(appender) => {
            let (w, g) = tracing_appender::non_blocking(appender);
            (Some(w), Some(g))
        }
        None => (None, None),
    };
    let registry = tracing_subscriber::registry().with(filter);
    let stderr = cfg!(debug_assertions).then(|| fmt::layer().with_target(true));
    let file_layer = writer.map(|w| fmt::layer().with_ansi(false).with_writer(w));
    let _ = registry.with(stderr).with(file_layer).try_init();
    guard
}
