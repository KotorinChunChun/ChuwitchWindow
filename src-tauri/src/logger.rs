use directories::ProjectDirs;
use std::fs;
use tracing_subscriber::{fmt, EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

pub fn setup_logger() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let proj_dirs = ProjectDirs::from("com", "kotorichun", "chuwitchwindow")?;
    let log_dir = proj_dirs.data_local_dir().join("logs");
    fs::create_dir_all(&log_dir).ok()?;

    let file_appender = tracing_appender::rolling::daily(&log_dir, "chuwitchwindow.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(EnvFilter::new("info,chuwitchwindow=debug"))
        .with(fmt::layer().with_writer(non_blocking))
        .with(fmt::layer().with_writer(std::io::stdout)) // also log to console
        .init();

    tracing::info!("Logger initialized at {:?}", log_dir);
    Some(guard)
}
