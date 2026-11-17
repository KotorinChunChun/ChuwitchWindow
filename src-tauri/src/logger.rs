use std::fs;
use std::path::PathBuf;
use tracing_subscriber::{fmt, EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

fn log_dir() -> Option<PathBuf> {
    Some(crate::config::project_dirs()?.data_local_dir().join("logs"))
}

/// 当日のログファイルのパス（tracing-appender の日次ローテーションの命名規則に合わせる）
fn today_log_path() -> Result<PathBuf, String> {
    let log_dir = log_dir().ok_or("Failed to get project dir")?;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    Ok(log_dir.join(format!("chuwitchwindow.log.{}", today)))
}

/// 保持するログファイル（日次）の数
const MAX_LOG_FILES: usize = 7;

pub fn setup_logger() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let log_dir = log_dir()?;
    fs::create_dir_all(&log_dir).ok()?;

    // 起動のたびにログを消すと、常駐中に CLI 引数付きで起動された 2 つ目のプロセス
    // （コマンドを転送してすぐ終了する）が常駐プロセスのログまで消してしまうため、
    // 削除は古い日付のファイルの自動削除に任せる。
    let file_appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("chuwitchwindow.log")
        .max_log_files(MAX_LOG_FILES)
        .build(&log_dir)
        .ok()?;
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
    let file_path = today_log_path()?;
    if file_path.exists() {
        fs::read_to_string(&file_path).map_err(|e| e.to_string())
    } else {
        Ok(String::from("No log file found for today."))
    }
}

pub fn clear_app_logs() -> Result<(), String> {
    let file_path = today_log_path()?;
    if file_path.exists() {
        // Windowsではロックされている可能性があるため、ファイルを空にする
        fs::File::create(&file_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}
