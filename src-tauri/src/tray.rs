use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let settings_i = MenuItem::with_id(app, "settings", "ChuwitchWindowの設定", true, None::<&str>)?;
    let restart_i = MenuItem::with_id(app, "restart", "再起動", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "終了", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&settings_i, &restart_i, &quit_i])?;

    let icon = app.default_window_icon().cloned().unwrap();

    let _tray = TrayIconBuilder::new()
        .tooltip("ウィンドウ入れ替えツール (ChuwitchWindow)")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => std::process::exit(0),
            "restart" => app.restart(),
            "settings" => {
                crate::show_main_window(app);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click { button: MouseButton::Left, .. } => {
                if let Some(window) = tray.app_handle().get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_always_on_top(true);
                    let _ = window.set_always_on_top(false);
                    let _ = window.set_focus();
                }
            }
            TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } => {
                crate::show_main_window(tray.app_handle());
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}
