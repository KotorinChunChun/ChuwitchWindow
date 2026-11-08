pub mod action;
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

/// 他のウィンドウの裏に隠れていても確実に前面へ出してフォーカスする
pub(crate) fn bring_to_front(window: &tauri::WebviewWindow) {
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_always_on_top(true);
    let _ = window.set_always_on_top(false);
    let _ = window.set_focus();
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        bring_to_front(&window);
    } else {
        // ウィンドウが存在しなければ再生成
        if let Ok(window) = tauri::WebviewWindowBuilder::new(
            app,
            "main",
            tauri::WebviewUrl::App("index.html".into())
        )
        .title("ChuwitchWindow 設定")
        .inner_size(800.0, 600.0)
        .resizable(true)
        .visible(true)
        .build() {
            bring_to_front(&window);
            // maximizedはビルド後に適用させる必要がある場合があるため
            let _ = window.maximize();
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
    let active_monitor = get_active_monitor_info();
    if let Some(monitor) = &active_monitor {
        arrange::set_target_hmonitor(monitor.hmonitor);
    }

    if let Some(window) = app.get_webview_window("arrange") {
        let _ = window.show();
        let _ = window.set_focus();

        // 中心に移動させる処理
        if let Some(monitor) = &active_monitor {
            let (x, y) = calc_center_position(monitor, ARRANGE_WINDOW_WIDTH, ARRANGE_WINDOW_HEIGHT);
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

        if let Some(monitor) = &active_monitor {
            let (x, y) = calc_center_position(monitor, ARRANGE_WINDOW_WIDTH, ARRANGE_WINDOW_HEIGHT);
            builder = builder.position(x, y);
        }

        let _ = builder.build();
    }
}

/// フォアグラウンドウィンドウが表示されているモニターの情報を返す
fn get_active_monitor_info() -> Option<crate::monitor::MonitorInfo> {
    let monitors = crate::monitor::get_all_monitors();
    let idx = crate::monitor::get_active_monitor_index(&monitors)?;
    monitors.into_iter().nth(idx)
}

pub fn hide_arrange_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("arrange") {
        let _ = window.close();
    }
}


/// CLI引数をディスパッチする共通関数。
/// 引数の中にアクションコマンドが含まれていれば実行し、1つでも実行したら true を返す。
fn dispatch_cli_action(app: &AppHandle, args: &[String]) -> bool {
    let mut matched = false;
    for action in args.iter().filter_map(|arg| action::Action::from_cli_arg(arg)) {
        action::execute(app, action);
        matched = true;
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
            let hidden = args.iter().any(|arg| matches!(arg.as_str(), "--autostart" | "--hide" | "-hide"));
            if !hidden {
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
            commands::check_autostart_registered_cmd,
            commands::get_all_monitors_cmd,
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
                tauri::WindowEvent::Destroyed => {
                    // ショートカット入力の録画中に設定画面を閉じられても、ホットキーが無効のまま残らないようにする
                    if window.label() == "main" {
                        hotkey::set_recording_state(false);
                    }
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
        .run(|_app_handle, event| {
            // 全ウィンドウが閉じられただけ（code: None）なら常駐を続ける。
            // app.exit() による明示的な終了（ワンショット実行・トレイの「終了」）は止めない。
            if let tauri::RunEvent::ExitRequested { code: None, api, .. } = event {
                api.prevent_exit();
            }
        });
}
