pub mod admin;
pub mod arrange;
pub mod config;
pub mod history;
pub mod hotkey;
pub mod hotplug;
pub mod logger;
pub mod logic;
pub mod monitor;
pub mod pin;
pub mod rule;
pub mod tray;
pub mod window;

use tauri::{AppHandle, Manager};
use tracing::info;

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

pub fn show_arrange_window(app: &AppHandle) {
    // ウィンドウを表示する「前」に対象モニターを記憶する。
    // この時点では arrange ウィンドウがまだ最前面でないため、
    // GetForegroundWindow() が正しいユーザーのウィンドウを指している。
    if let Some(monitor) = get_active_monitor_info() {
        arrange::set_target_hmonitor(monitor.hmonitor);
    }

    if let Some(window) = app.get_webview_window("arrange") {
        let _ = window.show();
        let _ = window.set_focus();
        
        // 中心に移動させる処理
        if let Some(monitor) = get_active_monitor_info() {
            let width = 1200.0;
            let height = 400.0;
            let center_x = monitor.monitor_area.left as f64 + (monitor.monitor_area.right - monitor.monitor_area.left) as f64 / 2.0;
            let center_y = monitor.monitor_area.top as f64 + (monitor.monitor_area.bottom - monitor.monitor_area.top) as f64 / 2.0;

            let x = center_x - (width / 2.0);
            let y = center_y - (height / 2.0);

            let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                x: x as i32,
                y: y as i32,
            }));
            let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize {
                width: width,
                height: height,
            }));
        }
    } else {
        let mut builder = tauri::WebviewWindowBuilder::new(
            app,
            "arrange",
            tauri::WebviewUrl::App("index.html#arrange".into())
        )
        .title("ChuwitchWindow 整列選択")
        .inner_size(1200.0, 400.0)
        .decorations(false)
        .always_on_top(true)
        .transparent(true)
        .resizable(false)
        .center();

        if let Some(monitor) = get_active_monitor_info() {
            let width = 1200.0;
            let height = 400.0;
            let center_x = monitor.monitor_area.left as f64 + (monitor.monitor_area.right - monitor.monitor_area.left) as f64 / 2.0;
            let center_y = monitor.monitor_area.top as f64 + (monitor.monitor_area.bottom - monitor.monitor_area.top) as f64 / 2.0;

            let x = center_x - (width / 2.0);
            let y = center_y - (height / 2.0);
            builder = builder.position(x, y);
        }

        let _ = builder.build();
    }
}

// ------------------------------------
// ArrangePopup 用のユーティリティ関数
// ------------------------------------
fn get_active_monitor_info() -> Option<crate::monitor::MonitorInfo> {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
        use windows::Win32::Graphics::Gdi::{MonitorFromWindow, MONITOR_DEFAULTTONEAREST};
        
        let hwnd = GetForegroundWindow();
        if hwnd.0 == 0 { return None; }
        
        let hmonitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        
        let monitors = crate::monitor::get_all_monitors();
        monitors.into_iter().find(|m| m.hmonitor == hmonitor.0 as isize)
    }
}

pub fn hide_arrange_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("arrange") {
        let _ = window.hide();
    }
}


#[tauri::command]
fn js_debug_log(message: String) {
    info!("[JS_LOG] {}", message);
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
fn get_app_logs_cmd() -> Result<String, String> {
    logger::get_app_logs()
}

#[tauri::command]
fn clear_app_logs_cmd() -> Result<(), String> {
    logger::clear_app_logs()
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

#[tauri::command]
fn get_window_list_cmd() -> Vec<WindowUIData> {
    let mut result = Vec::new();
    let wins = crate::window::get_target_windows(false, false); 
    
    for w in wins {
        result.push(WindowUIData {
            hwnd: w.hwnd,
            title: w.title,
            process_name: w.process_name,
            class_name: w.class_name,
            is_pinned: crate::pin::is_pinned(w.hwnd),
            style: w.style,
            ex_style: w.ex_style,
        });
    }
    result
}

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn show_arrange_window_cmd(app: AppHandle) {
    show_arrange_window(&app);
}

#[tauri::command]
fn hide_arrange_window_cmd(app: AppHandle) {
    hide_arrange_window(&app);
}

#[tauri::command]
fn exec_arrange_cmd(arrange_type: String, app: AppHandle) {
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
    hide_arrange_window(&app);
}

#[tauri::command]
fn export_config_cmd(path: String) -> Result<(), String> {
    let config = config::load_config();
    let json = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

#[tauri::command]
fn import_config_cmd(path: String) -> Result<config::AppConfig, String> {
    let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let new_config: config::AppConfig = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    config::save_config(&new_config);
    Ok(new_config)
}

#[tauri::command]
async fn register_to_path_cmd() -> Result<(), String> {
    admin::register_to_path()
}

#[tauri::command]
async fn check_path_registered_cmd() -> Result<bool, String> {
    admin::check_path_registered()
}

#[tauri::command]
async fn unregister_from_path_cmd() -> Result<(), String> {
    admin::unregister_from_path()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(history::HistoryState::new())
        .setup(|app| {
            let args: Vec<String> = std::env::args().collect();
            tracing::info!("App starting with args: {:?}", args);

            let mut is_oneshot = false;
            for arg in &args {
                if arg == "--rotate-cw" {
                    crate::logic::handle_rotate(app.handle(), true);
                    is_oneshot = true;
                } else if arg == "--rotate-ccw" {
                    crate::logic::handle_rotate(app.handle(), false);
                    is_oneshot = true;
                } else if arg == "--undo" {
                    crate::logic::handle_undo(app.handle());
                    is_oneshot = true;
                } else if arg == "--pin-toggle" {
                    crate::pin::toggle_pin();
                    is_oneshot = true;
                } else if arg == "--escape" {
                    crate::logic::handle_escape(app.handle());
                    is_oneshot = true;
                } else if arg == "--gather" {
                    crate::logic::handle_gather(app.handle());
                    is_oneshot = true;
                } else if let Some(target_str) = arg.strip_prefix("--swap=") {
                    if let Ok(target) = target_str.parse::<u32>() {
                        crate::logic::handle_swap_target(app.handle(), target);
                        is_oneshot = true;
                    }
                }
            }

            if is_oneshot {
                // ワンショット実行の場合は常駐せずに即終了
                app.handle().exit(0);
                return Ok(());
            }

            hotplug::start_hotplug_listener(app.handle().clone());
            hotkey::init(app.handle().clone());
            let _ = tray::setup_tray(app.handle());

            // 引数に --autostart, --hide, -hide が含まれていない場合のみウィンドウを表示（GUIからの通常起動）
            if !args.contains(&"--autostart".to_string()) 
                && !args.contains(&"--hide".to_string()) 
                && !args.contains(&"-hide".to_string()) {
                crate::show_main_window(app.handle());
            } else {
                // 自動起動・非表示時はメインウィンドウを破棄してメモリ削減
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.close();
                }
            }
            Ok(())
        })
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            tracing::info!("Received IPC args from secondary instance: {:?}", args);
            let mut is_cli_cmd = false;
            for arg in &args {
                if arg == "--rotate-cw" {
                    crate::logic::handle_rotate(app, true);
                    is_cli_cmd = true;
                } else if arg == "--rotate-ccw" {
                    crate::logic::handle_rotate(app, false);
                    is_cli_cmd = true;
                } else if arg == "--undo" {
                    crate::logic::handle_undo(app);
                    is_cli_cmd = true;
                } else if arg == "--pin-toggle" {
                    crate::pin::toggle_pin();
                    use tauri::Emitter;
                    let _ = app.emit("pin-toggled", ());
                    is_cli_cmd = true;
                } else if arg == "--escape" {
                    crate::logic::handle_escape(app);
                    is_cli_cmd = true;
                } else if arg == "--gather" {
                    crate::logic::handle_gather(app);
                    is_cli_cmd = true;
                } else if let Some(target_str) = arg.strip_prefix("--swap=") {
                    if let Ok(target) = target_str.parse::<u32>() {
                        crate::logic::handle_swap_target(app, target);
                        is_cli_cmd = true;
                    }
                }
            }

            // CLIコマンドでなければ通常のGUI呼出とみなしてメイン画面を表示
            if !is_cli_cmd {
                crate::show_main_window(app);
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .plugin(tauri_plugin_dialog::init())
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
            get_window_list_cmd,
            get_app_logs_cmd,
            clear_app_logs_cmd,
            show_arrange_window_cmd,
            hide_arrange_window_cmd,
            exec_arrange_cmd,
            export_config_cmd,
            import_config_cmd,
            register_to_path_cmd,
            check_path_registered_cmd,
            unregister_from_path_cmd,
            js_debug_log,
        ])
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { .. } => {
                    // 何もしなければそのままWindowがClose（破棄）される
                }
                tauri::WindowEvent::Focused(focused) => {
                    // フォーカスが外れた場合、ArrangePopupなら非表示にする
                    if !focused && window.label() == "arrange" {
                        let _ = window.hide();
                    }
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
