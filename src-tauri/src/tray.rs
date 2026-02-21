use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let settings_i = MenuItem::with_id(app, "settings", "設定（プレビュー）", true, None::<&str>)?;
    let restart_i = MenuItem::with_id(app, "restart", "再起動", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "終了", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&settings_i, &restart_i, &quit_i])?;

    let icon = app.default_window_icon().cloned().unwrap();

    let _tray = TrayIconBuilder::new()
        .tooltip("ウィンドウ入れ替えツール (ChuwitchWindow)")
        .icon(icon)
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => app.exit(0),
            "restart" => app.restart(),
            "settings" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::DoubleClick { .. } = event {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}
