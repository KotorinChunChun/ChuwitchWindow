//! README撮影専用。実画面を架空のモニター・ウィンドウと隔離設定で開く。
//! 通常起動、フック、トレイ、OS予約、ウィンドウ操作は呼び出さない。
use chuwitchwindow::config::AppConfig;
use serde_json::{json, Value};
use std::{path::{Path, PathBuf}, sync::Mutex};
use tauri::Manager;

struct Preview { root: PathBuf, config: Mutex<AppConfig> }
impl Preview {
    fn create(root: &Path) -> Result<Self, String> {
        if !root.is_absolute() { return Err("絶対パスの新規フォルダーを指定してください".into()); }
        std::fs::create_dir(root).map_err(|e| e.to_string())?;
        Ok(Self { root: root.into(), config: Mutex::new(AppConfig::default()) })
    }
    fn save(&self, value: AppConfig) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?;
        std::fs::write(self.root.join("config.json"), bytes).map_err(|e| e.to_string())?;
        *self.config.lock().unwrap() = value;
        Ok(())
    }
}

#[tauri::command]
fn get_config(state: tauri::State<Preview>) -> AppConfig { state.config.lock().unwrap().clone() }
#[tauri::command]
fn save_config(new_config: AppConfig, state: tauri::State<Preview>) -> Result<(), String> { state.save(new_config) }
#[tauri::command]
fn is_user_an_admin() -> bool { false }
#[tauri::command]
fn check_path_registered_cmd() -> bool { false }
#[tauri::command]
fn check_autostart_registered_cmd() -> Value { json!({"is_registered":false,"is_same_path":false,"registered_path":null}) }
#[tauri::command]
fn check_hotkey_conflict_cmd() -> bool { false }
#[tauri::command]
fn set_recording_state_cmd() {}
#[tauri::command]
fn js_debug_log() {}
#[tauri::command]
fn get_all_monitors_cmd() -> Value {
    json!([
        {"hmonitor":1,"name":"SampleDisplay1","display_number":1,"manufacturer":"Sample","serial_number":"","work_area":{"left":0,"top":0,"right":1920,"bottom":1040},"monitor_area":{"left":0,"top":0,"right":1920,"bottom":1080},"is_primary":true},
        {"hmonitor":2,"name":"SampleDisplay2","display_number":2,"manufacturer":"Sample","serial_number":"","work_area":{"left":1920,"top":0,"right":3840,"bottom":1040},"monitor_area":{"left":1920,"top":0,"right":3840,"bottom":1080},"is_primary":false}
    ])
}
#[tauri::command]
fn get_window_list_cmd() -> Value {
    json!([{"hwnd":1,"title":"撮影用メモ","process_name":"SampleViewer.exe","class_name":"SampleWindow","is_pinned":true,"is_fullscreen":false,"is_minimized":false,"is_maximized":false,"style":0,"ex_style":0}])
}
#[tauri::command]
fn get_app_logs_cmd() -> String { "撮影用の設定画面です。実ウィンドウ・OS予約への操作は行いません。".into() }

fn main() {
    let root = std::env::args().nth(1).expect("撮影用の新規フォルダーが必要です");
    let preview = Preview::create(Path::new(&root)).expect("撮影領域を作成できません");
    let mut context = tauri::generate_context!();
    context.config_mut().identifier = "com.kotorichun.chuwitchwindow.manual-preview".into();
    context.config_mut().app.windows.clear();
    tauri::Builder::default()
        .manage(preview)
        .setup(|app| {
            let data = app.state::<Preview>().root.join("webview");
            tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App("index.html".into()))
                .title("撮影用 - ChuwitchWindow - ver 0.2.1")
                .data_directory(data).inner_size(1200.0, 760.0).build()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_config, save_config, is_user_an_admin,
            get_all_monitors_cmd, get_window_list_cmd, check_path_registered_cmd,
            check_autostart_registered_cmd, check_hotkey_conflict_cmd, set_recording_state_cmd,
            js_debug_log, get_app_logs_cmd])
        .run(context).expect("撮影画面を開けませんでした");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_preserves_existing_directory_and_saves_only_its_own_config() {
        let root = std::env::temp_dir().join(format!("chuwitch-preview-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        assert!(Preview::create(Path::new("relative")).is_err());
        let preview = Preview::create(&root).unwrap();
        let mut config = AppConfig::default();
        config.swap_within_groups = true;
        preview.save(config).unwrap();
        let reloaded: AppConfig = serde_json::from_slice(&std::fs::read(root.join("config.json")).unwrap()).unwrap();
        assert!(reloaded.swap_within_groups);
        assert!(!reloaded.run_on_startup);
        assert!(Preview::create(&root).is_err());
        // この試験が生成した2項目だけを削除する。
        std::fs::remove_file(root.join("config.json")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
