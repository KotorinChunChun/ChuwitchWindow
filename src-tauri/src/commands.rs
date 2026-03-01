//! フロントエンドから呼び出される Tauri コマンドの定義
//!
//! `lib.rs` の肥大化を防ぐため、`#[tauri::command]` 関数をこのモジュールに集約している。

use tauri::AppHandle;
use tracing::info;

use crate::{admin, arrange, config, hotkey, logger, logic, monitor, pin};

// ------------------------------------
// データ構造
// ------------------------------------

/// フロントエンドのウィンドウ一覧表示用データ
#[derive(serde::Serialize)]
pub struct WindowUIData {
    pub hwnd: isize,
    pub title: String,
    pub process_name: String,
    pub class_name: String,
    pub is_pinned: bool,
    pub style: u32,
    pub ex_style: u32,
}

// ------------------------------------
// コマンド関数
// ------------------------------------

#[tauri::command]
pub fn js_debug_log(message: String) {
    info!("[JS_LOG] {}", message);
}

#[tauri::command]
pub fn open_url(url: String) {
    // WindowsでURLを開く標準的な方法 (startコマンド)
    let _ = std::process::Command::new("cmd")
        .args(&["/C", "start", &url])
        .spawn();
}

#[tauri::command]
pub fn get_config() -> config::AppConfig {
    config::load_config()
}

#[tauri::command]
pub fn save_config(new_config: config::AppConfig) -> Result<(), String> {
    config::save_config(&new_config);
    hotkey::reload_hotkeys();
    Ok(())
}

#[tauri::command]
pub fn get_app_logs_cmd() -> Result<String, String> {
    logger::get_app_logs()
}

#[tauri::command]
pub fn clear_app_logs_cmd() -> Result<(), String> {
    logger::clear_app_logs()
}

#[tauri::command]
pub fn reset_config_cmd() -> Result<config::AppConfig, String> {
    config::reset_config();
    hotkey::reload_hotkeys();
    Ok(config::load_config())
}

#[tauri::command]
pub fn is_user_an_admin() -> bool {
    admin::is_user_an_admin()
}

#[tauri::command]
pub fn restart_as_admin() -> Result<(), String> {
    admin::restart_as_admin().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn sync_admin_startup(enable: bool) -> Result<(), String> {
    admin::sync_admin_startup(enable).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_all_monitors_cmd() -> Vec<monitor::MonitorInfo> {
    monitor::get_all_monitors()
}

#[tauri::command]
pub fn trigger_action(action: String, app: AppHandle) {
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
pub fn check_hotkey_conflict_cmd(hotkey_str: String) -> bool {
    hotkey::check_hotkey_conflict(hotkey_str)
}

#[tauri::command]
pub fn set_recording_state_cmd(is_recording: bool) {
    hotkey::set_recording_state(is_recording)
}

#[tauri::command]
pub fn get_window_list_cmd() -> Vec<WindowUIData> {
    let mut result = Vec::new();
    let wins = crate::window::get_target_windows(false, false); 
    
    for w in wins {
        result.push(WindowUIData {
            hwnd: w.hwnd,
            title: w.title,
            process_name: w.process_name,
            class_name: w.class_name,
            is_pinned: pin::is_pinned(w.hwnd),
            style: w.style,
            ex_style: w.ex_style,
        });
    }
    result
}

#[tauri::command]
pub fn show_arrange_window_cmd(app: AppHandle) {
    crate::show_arrange_window(&app);
}

#[tauri::command]
pub fn hide_arrange_window_cmd(app: AppHandle) {
    crate::hide_arrange_window(&app);
}

#[tauri::command]
pub fn exec_arrange_cmd(arrange_type: String, app: AppHandle) {
    info!("Exec [exec_arrange_cmd]: type_str='{}'", arrange_type);
    
    let typ = match arrange_type.as_str() {
        "Grid" => arrange::ArrangeType::Grid,
        "Vertical" => arrange::ArrangeType::Vertical,
        "Horizontal" => arrange::ArrangeType::Horizontal,
        "Cascade" => arrange::ArrangeType::Cascade,
        _ => {
            info!("[exec_arrange_cmd] 不明な整列タイプ: {}", arrange_type);
            arrange::ArrangeType::Grid
        }
    };

    arrange::handle_arrange(&app, typ);
    crate::hide_arrange_window(&app);
}

#[tauri::command]
pub fn export_config_cmd(path: String) -> Result<(), String> {
    let config = config::load_config();
    let json = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_config_cmd(path: String) -> Result<config::AppConfig, String> {
    let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let new_config: config::AppConfig = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    config::save_config(&new_config);
    Ok(new_config)
}

#[tauri::command]
pub async fn register_to_path_cmd() -> Result<(), String> {
    admin::register_to_path()
}

#[tauri::command]
pub async fn check_path_registered_cmd() -> Result<bool, String> {
    admin::check_path_registered()
}

#[tauri::command]
pub async fn unregister_from_path_cmd() -> Result<(), String> {
    admin::unregister_from_path()
}
