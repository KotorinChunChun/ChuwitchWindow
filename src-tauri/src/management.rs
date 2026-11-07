//! KotorinToys管理契約2。単体の起動・設定や予約は既存処理に任せる。
mod pipe;

use crate::window_title::APP_VERSION;
use anyhow::ensure;
use pipe::ManagementPipe;
use serde::Deserialize;
use serde_json::json;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tauri::AppHandle;

const TOOL_ID: &str = "chuwitch-window";
const MAX_REQUEST_BYTES: usize = 8192;

pub fn start(app: AppHandle, args: &[String]) -> anyhow::Result<()> {
    let origin = match args
        .windows(2)
        .find(|pair| pair[0] == "--launch-origin")
        .map(|pair| pair[1].as_str())
    {
        Some("host_manual") => "host_manual",
        _ => "standalone",
    };
    let (ready, started) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("chuwitch-management".into())
        .spawn(move || {
            let pipe = match ManagementPipe::create(TOOL_ID) {
                Ok(pipe) => pipe,
                Err(error) => {
                    let _ = ready.send(Err(error));
                    return;
                }
            };
            let _ = ready.send(Ok(()));
            loop {
                if pipe.accept(Duration::from_secs(1)).is_err() {
                    // 接続と期限が競合した場合も次の要求を受付可能にする。
                    pipe.disconnect();
                    continue;
                }
                let deadline = Instant::now() + Duration::from_millis(1500);
                let stop = match handle(&pipe, &app, origin, deadline) {
                    Ok(stop) => {
                        pipe.wait_for_client_close(deadline);
                        stop
                    }
                    Err(error) => {
                        tracing::debug!("管理要求を処理できませんでした: {error}");
                        false
                    }
                };
                pipe.disconnect();
                if stop {
                    app.exit(0);
                    break;
                }
            }
        })?;
    started.recv_timeout(Duration::from_secs(2))?
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Request {
    command: String,
    #[serde(default)]
    request_id: String,
    expected_pid: Option<u32>,
}

fn handle(
    pipe: &ManagementPipe,
    app: &AppHandle,
    origin: &str,
    deadline: Instant,
) -> anyhow::Result<bool> {
    ensure!(pipe.same_session()?, "別セッションの管理要求です");
    let line = pipe.read_line(deadline)?;
    let request = if line == "status" {
        Request {
            command: line,
            request_id: String::new(),
            expected_pid: None,
        }
    } else {
        serde_json::from_str(&line)?
    };
    ensure!(request.request_id.len() <= 128, "要求IDが長すぎます");
    let pid = std::process::id();
    let (result, detail) = match request.command.as_str() {
        _ if request.expected_pid.is_some_and(|expected| expected != pid) => {
            ("refused", "対象PIDが変わりました。")
        }
        "show" | "stop" if request.expected_pid.is_none() => {
            ("refused", "操作には対象PIDが必要です。")
        }
        "status" => ("completed", ""),
        "show" if show(app, deadline) => ("completed", ""),
        "show" => ("failed", "設定画面の表示を確認できませんでした。"),
        "stop" => ("accepted", ""),
        _ => ("unsupported", "未対応の操作です。"),
    };
    let reply = json!({
        "protocolVersion": 2,
        "requestId": request.request_id,
        "pid": pid,
        "ready": true,
        "version": APP_VERSION,
        "launchOrigin": origin,
        "capabilities": ["status", "show", "stop"],
        "result": result,
        "detail": detail
    });
    pipe.write_line(&reply.to_string(), deadline)?;
    Ok(result == "accepted")
}

fn show(app: &AppHandle, deadline: Instant) -> bool {
    let window_app = app.clone();
    let (sender, receiver) = mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        if Instant::now() < deadline {
            let _ = sender.send(crate::open_main_window(&window_app).is_ok());
        }
    })
    .is_ok()
        && receiver
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap_or(false)
}
