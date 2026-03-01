pub mod admin;
pub mod arrange;
pub mod commands;
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

/// arrange ウィンドウのデフォルトサイズ
const ARRANGE_WINDOW_WIDTH: f64 = 1200.0;
const ARRANGE_WINDOW_HEIGHT: f64 = 400.0;

/// モニター領域の中心にウィンドウを配置するための左上座標を計算する
fn calc_center_position(monitor: &crate::monitor::MonitorInfo, width: f64, height: f64) -> (f64, f64) {
    let center_x = monitor.monitor_area.left as f64
        + (monitor.monitor_area.right - monitor.monitor_area.left) as f64 / 2.0;
    let center_y = monitor.monitor_area.top as f64
        + (monitor.monitor_area.bottom - monitor.monitor_area.top) as f64 / 2.0;
    (center_x - width / 2.0, center_y - height / 2.0)
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
            let (x, y) = calc_center_position(&monitor, ARRANGE_WINDOW_WIDTH, ARRANGE_WINDOW_HEIGHT);
            let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                x: x as i32,
                y: y as i32,
            }));
            let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize {
                width: ARRANGE_WINDOW_WIDTH,
                height: ARRANGE_WINDOW_HEIGHT,
            }));
        }
    } else {
        let mut builder = tauri::WebviewWindowBuilder::new(
            app,
            "arrange",
            tauri::WebviewUrl::App("index.html#arrange".into())
        )
        .title("ChuwitchWindow 整列選択")
        .inner_size(ARRANGE_WINDOW_WIDTH, ARRANGE_WINDOW_HEIGHT)
        .decorations(false)
        .always_on_top(true)
        .transparent(true)
        .resizable(false)
        .center();

        if let Some(monitor) = get_active_monitor_info() {
            let (x, y) = calc_center_position(&monitor, ARRANGE_WINDOW_WIDTH, ARRANGE_WINDOW_HEIGHT);
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


/// CLI引数をディスパッチする共通関数。
/// 引数の中にアクションコマンドが含まれていれば実行し、1つでも実行したら true を返す。
fn dispatch_cli_action(app: &AppHandle, args: &[String]) -> bool {
    use tauri::Emitter;
    let mut matched = false;
    for arg in args {
        if arg == "--rotate-cw" {
            crate::logic::handle_rotate(app, true);
            matched = true;
        } else if arg == "--rotate-ccw" {
            crate::logic::handle_rotate(app, false);
            matched = true;
        } else if arg == "--undo" {
            crate::logic::handle_undo(app);
            matched = true;
        } else if arg == "--pin-toggle" {
            crate::pin::toggle_pin();
            let _ = app.emit("pin-toggled", ());
            matched = true;
        } else if arg == "--escape" {
            crate::logic::handle_escape(app);
            matched = true;
        } else if arg == "--gather" {
            crate::logic::handle_gather(app);
            matched = true;
        } else if let Some(target_str) = arg.strip_prefix("--swap=") {
            if let Ok(target) = target_str.parse::<u32>() {
                crate::logic::handle_swap_target(app, target);
                matched = true;
            }
        }
    }
    matched
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(history::HistoryState::new())
        .setup(|app| {
            let args: Vec<String> = std::env::args().collect();
            tracing::info!("App starting with args: {:?}", args);

            if dispatch_cli_action(app.handle(), &args) {
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

            // CLIコマンドでなければ通常のGUI呼出とみなしてメイン画面を表示
            if !dispatch_cli_action(app, &args) {
                crate::show_main_window(app);
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::is_user_an_admin,
            commands::restart_as_admin,
            commands::sync_admin_startup,
            commands::get_all_monitors_cmd,
            commands::trigger_action,
            commands::check_hotkey_conflict_cmd,
            commands::set_recording_state_cmd,
            commands::reset_config_cmd,
            commands::open_url,
            commands::get_window_list_cmd,
            commands::get_app_logs_cmd,
            commands::clear_app_logs_cmd,
            commands::show_arrange_window_cmd,
            commands::hide_arrange_window_cmd,
            commands::exec_arrange_cmd,
            commands::export_config_cmd,
            commands::import_config_cmd,
            commands::register_to_path_cmd,
            commands::check_path_registered_cmd,
            commands::unregister_from_path_cmd,
            commands::js_debug_log,
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
