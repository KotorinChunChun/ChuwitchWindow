pub mod action;
pub mod admin;
mod alt_drag;
pub mod arrange;
pub mod commands;
pub mod config;
pub mod history;
pub mod hotkey;
pub mod hotplug;
pub mod logger;
pub mod logic;
mod management;
pub mod monitor;
pub mod pin;
pub mod rule;
pub mod tray;
pub mod window;
mod window_title;

#[cfg(test)]
pub mod test_support;

use tauri::{AppHandle, Manager};
use window_title::WindowTitle;

pub fn show_main_window(app: &AppHandle) {
    let _ = open_main_window(app);
}

fn open_main_window(app: &AppHandle) -> tauri::Result<()> {
    let window = match app.get_webview_window("main") {
        Some(window) => window,
        None => tauri::WebviewWindowBuilder::new(
            app,
            "main",
            tauri::WebviewUrl::App("index.html".into()),
        )
        .inner_size(800.0, 600.0)
        .resizable(true)
        .maximized(true)
        .visible(false)
        .build()?,
    };
    window.set_title(&WindowTitle::create(""))?;
    window.show()?;
    let _ = window.unminimize();
    let _ = window.set_always_on_top(true);
    let _ = window.set_always_on_top(false);
    let _ = window.set_focus();
    Ok(())
}

/// arrange ウィンドウのデフォルトサイズ
const ARRANGE_WINDOW_WIDTH: f64 = 1200.0;
const ARRANGE_WINDOW_HEIGHT: f64 = 400.0;

/// モニター領域の中心にウィンドウを配置するための左上座標を計算する
fn calc_center_position(
    monitor: &crate::monitor::MonitorInfo,
    width: f64,
    height: f64,
) -> (f64, f64) {
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
            let (x, y) =
                calc_center_position(&monitor, ARRANGE_WINDOW_WIDTH, ARRANGE_WINDOW_HEIGHT);
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
            tauri::WebviewUrl::App("index.html#arrange".into()),
        )
        .title(WindowTitle::create("整列選択"))
        .inner_size(ARRANGE_WINDOW_WIDTH, ARRANGE_WINDOW_HEIGHT)
        .decorations(false)
        .always_on_top(true)
        .transparent(true)
        .resizable(false)
        .center();

        if let Some(monitor) = get_active_monitor_info() {
            let (x, y) =
                calc_center_position(&monitor, ARRANGE_WINDOW_WIDTH, ARRANGE_WINDOW_HEIGHT);
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
        use windows::Win32::Graphics::Gdi::{MonitorFromWindow, MONITOR_DEFAULTTONEAREST};
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

        let hwnd = GetForegroundWindow();
        if hwnd.0 == 0 {
            return None;
        }

        let hmonitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);

        let monitors = crate::monitor::get_all_monitors();
        monitors
            .into_iter()
            .find(|m| m.hmonitor == hmonitor.0 as isize)
    }
}

pub fn hide_arrange_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("arrange") {
        let _ = window.close();
    }
}

/// CLI引数をディスパッチする共通関数。
/// 引数の中にアクションコマンドが含まれていれば実行し、1つでも実行したら true を返す。
fn dispatch_cli_action(
    app: &AppHandle,
    args: &[String],
) -> Result<bool, crate::action::ParseError> {
    crate::action::dispatch_cli_action(app, args)
}

fn hidden_launch(args: &[String]) -> bool {
    args.iter().skip(1).any(|arg| {
        matches!(
            arg.as_str(),
            "--startup" | "--autostart" | "--hidden" | "--hide" | "-hide"
        )
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(history::HistoryState::new())
        .setup(|app| {
            let args: Vec<String> = std::env::args().collect();
            tracing::info!("App starting with args: {:?}", args);

            match dispatch_cli_action(app.handle(), &args) {
                Ok(true) => {
                    // ワンショット実行の場合は常駐せずに即終了
                    app.handle().exit(0);
                    return Ok(());
                }
                Ok(false) => {}
                Err(error) => {
                    tracing::error!("CLI引数の解析に失敗しました: {}", error.message);
                    eprintln!("{}", error.message);
                    app.handle().exit(1);
                    return Ok(());
                }
            }

            hotplug::start_hotplug_listener(app.handle().clone());
            hotkey::init(app.handle().clone());
            alt_drag::init(&config::load_config());
            tray::setup_tray(app.handle())?;

            if hidden_launch(&args) {
                // 自動起動・非表示時はメインウィンドウを破棄してメモリ削減
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.close();
                }
            } else {
                open_main_window(app.handle())?;
            }
            // 管理窓口の失敗で単体利用を妨げず、親には準備完了を返さない。
            if let Err(error) = management::start(app.handle().clone(), &args) {
                tracing::warn!("管理窓口を開始できませんでした: {error}");
            }
            Ok(())
        })
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            tracing::info!("Received IPC args from secondary instance: {:?}", args);

            // CLIコマンドでなければ通常のGUI呼出とみなしてメイン画面を表示
            match dispatch_cli_action(app, &args) {
                Ok(true) => {}
                Ok(false) if !hidden_launch(&args) => crate::show_main_window(app),
                Ok(false) => {}
                Err(error) => {
                    // 第二インスタンスの不正な引数で常駐中の本体を終了させない。
                    tracing::error!("第二インスタンスのCLI引数を拒否しました: {}", error.message);
                }
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
            commands::check_autostart_registered_cmd,
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
                    // フォーカスが外れた場合、ArrangePopupなら破棄してメモリを解放する
                    if !focused && window.label() == "arrange" {
                        let _ = window.close();
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
            tauri::RunEvent::ExitRequested { code: None, api, .. } => {
                api.prevent_exit();
            }
            _ => {}
        });
}
