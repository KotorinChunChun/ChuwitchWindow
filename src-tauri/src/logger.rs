use directories::ProjectDirs;
use std::fs;
use tracing_subscriber::{fmt, EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

pub fn setup_logger() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let proj_dirs = ProjectDirs::from("com", "kotorichun", "chuwitchwindow")?;
    let log_dir = proj_dirs.data_local_dir().join("logs");
    
    // 起動時に既存のログをクリーンアップする
    let _ = fs::remove_dir_all(&log_dir);
    fs::create_dir_all(&log_dir).ok()?;

    let file_appender = tracing_appender::rolling::daily(&log_dir, "chuwitchwindow.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // ログフォーマットを読みやすく整形 (prettyフォーマットで余白を追加)
    tracing_subscriber::registry()
        .with(EnvFilter::new("info,chuwitchwindow=debug"))
        .with(
            fmt::layer()
                .pretty()
                .with_ansi(false) // ログファイルには色コードを出力しない
                .with_writer(non_blocking),
        )
        .with(
            fmt::layer()
                .pretty()
                .with_writer(std::io::stdout),
        )
        .init();

    tracing::info!("Logger initialized at {:?}", log_dir);
    Some(guard)
}

pub fn get_app_logs() -> Result<String, String> {
    let proj_dirs = ProjectDirs::from("com", "kotorichun", "chuwitchwindow")
        .ok_or("Failed to get project dir")?;
    let log_dir = proj_dirs.data_local_dir().join("logs");
    
    // Read the current date's log file based on tracing-appender format
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let file_path = log_dir.join(format!("chuwitchwindow.log.{}", today));
    
    if file_path.exists() {
        fs::read_to_string(&file_path).map_err(|e| e.to_string())
    } else {
        // Fallback for rotation if exact date match is not found, or just return empty
        Ok(String::from("No log file found for today."))
    }
}

pub fn clear_app_logs() -> Result<(), String> {
    let proj_dirs = ProjectDirs::from("com", "kotorichun", "chuwitchwindow")
        .ok_or("Failed to get project dir")?;
    let log_dir = proj_dirs.data_local_dir().join("logs");
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let file_path = log_dir.join(format!("chuwitchwindow.log.{}", today));
    
    if file_path.exists() {
        // Windowsではロックされている可能性があるため、ファイルを空にする
        std::fs::File::create(&file_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}
