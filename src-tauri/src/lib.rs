pub mod admin;
pub mod config;
pub mod history;
pub mod hotkey;
pub mod hotplug;
pub mod logger;
pub mod logic;
pub mod monitor;
pub mod tray;
pub mod window;

use tauri::{AppHandle, Manager};

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_always_on_top(true);
        let _ = window.set_always_on_top(false);
        let _ = window.set_focus();
    } else {
        // ウィンドウが存在しなければ再生成
        if let Ok(builder) = tauri::WebviewWindowBuilder::new(
            app,
            "main",
            tauri::WebviewUrl::App("index.html".into())
        )
        .title("ChuwitchWindow 設定")
        .inner_size(800.0, 600.0)
        .resizable(true)
        .visible(true)
        .build() {
            let _ = builder.set_always_on_top(true);
            let _ = builder.set_always_on_top(false);
            let _ = builder.set_focus();
            // maximizedはビルド後に適用させる必要がある場合があるため
            let _ = builder.maximize();
        }
    }
}

#[tauri::command]
fn open_url(url: String) {
    // WindowsでURLを開く標準的な方法 (startコマンド)
    let _ = std::process::Command::new("cmd")
        .args(&["/C", "start", &url])
        .spawn();
}

#[tauri::command]
fn get_config() -> config::AppConfig {
    config::load_config()
}

#[tauri::command]
fn save_config(new_config: config::AppConfig) -> Result<(), String> {
    config::save_config(&new_config);
    crate::hotkey::reload_hotkeys();
    Ok(())
}

#[tauri::command]
fn reset_config_cmd() -> Result<config::AppConfig, String> {
    config::reset_config();
    crate::hotkey::reload_hotkeys();
    Ok(config::load_config())
}

#[tauri::command]
fn is_user_an_admin() -> bool {
    admin::is_user_an_admin()
}

#[tauri::command]
fn restart_as_admin() -> Result<(), String> {
    admin::restart_as_admin().map_err(|e| e.to_string())
}

#[tauri::command]
fn sync_admin_startup(enable: bool) -> Result<(), String> {
    admin::sync_admin_startup(enable).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_all_monitors_cmd() -> Vec<monitor::MonitorInfo> {
    monitor::get_all_monitors()
}

#[tauri::command]
fn trigger_action(action: String, app: AppHandle) {
    if action == "rotate_cw" {
        logic::handle_rotate(&app, true);
    } else if action == "rotate_ccw" {
        logic::handle_rotate(&app, false);
    } else if action == "undo" {
        logic::handle_undo(&app);
    } else if action.starts_with("swap_target_") {
        if let Ok(target_num) = action.replace("swap_target_", "").parse::<u32>() {
            logic::handle_swap_target(&app, target_num);
        }
    }
}

#[tauri::command]
fn check_hotkey_conflict_cmd(hotkey_str: String) -> bool {
    hotkey::check_hotkey_conflict(hotkey_str)
}

#[tauri::command]
fn set_recording_state_cmd(is_recording: bool) {
    hotkey::set_recording_state(is_recording)
}

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(history::HistoryState::new())
        .setup(|app| {
            hotplug::start_hotplug_listener(app.handle().clone());
            hotkey::init(app.handle().clone());
            let _ = tray::setup_tray(app.handle());

            // 引数に --autostart が含まれていない場合のみウィンドウを表示（GUIからの通常起動）
            let args: Vec<String> = std::env::args().collect();
            if !args.contains(&"--autostart".to_string()) {
                crate::show_main_window(app.handle());
            } else {
                // 自動起動時はメインウィンドウを破棄してメモリ削減
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.close();
                }
            }
            Ok(())
        })
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 二重起動時は既存のメインウィンドウを表示してフォーカス
            crate::show_main_window(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .invoke_handler(tauri::generate_handler![
            greet,
            get_config,
            save_config,
            is_user_an_admin,
            restart_as_admin,
            sync_admin_startup,
            get_all_monitors_cmd,
            trigger_action,
            check_hotkey_conflict_cmd,
            set_recording_state_cmd,
            reset_config_cmd,
            open_url,
        ])
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { .. } => {
                    // 何もしなければそのままWindowがClose（破棄）される
                }
                tauri::WindowEvent::Resized(_) => {
                    if window.label() == "main" {
                        if window.is_minimized().unwrap_or(false) {
                            let _ = window.close(); // 最小化時に破棄
                        }
                    }
                }
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| match event {
            tauri::RunEvent::ExitRequested { api, .. } => {
                api.prevent_exit();
            }
            _ => {}
        });
}
