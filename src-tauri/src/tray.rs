use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

#[derive(Clone, Copy)]
enum TrayAction {
    OpenSettings,
    FocusSettings,
    Restart,
    Quit,
}

fn menu_action(id: &str) -> Option<TrayAction> {
    match id {
        "settings" => Some(TrayAction::OpenSettings),
        "restart" => Some(TrayAction::Restart),
        "quit" => Some(TrayAction::Quit),
        _ => None,
    }
}

fn click_action(button: MouseButton, double_click: bool) -> Option<TrayAction> {
    (button == MouseButton::Left).then_some(if double_click {
        TrayAction::OpenSettings
    } else {
        TrayAction::FocusSettings
    })
}

fn run_action(
    action: Option<TrayAction>,
    show_settings: impl FnOnce(bool),
    restart: impl FnOnce(),
    quit: impl FnOnce(),
) {
    match action {
        Some(TrayAction::OpenSettings) => show_settings(true),
        Some(TrayAction::FocusSettings) => show_settings(false),
        Some(TrayAction::Restart) => restart(),
        Some(TrayAction::Quit) => quit(),
        None => {}
    }
}

fn show_settings(app: &AppHandle, create: bool) {
    if create {
        let _ = crate::open_main_window(app);
    } else if let Some(window) = app.get_webview_window("main") {
        let _ = crate::present_settings_window(&window);
    }
}

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let settings_i =
        MenuItem::with_id(app, "settings", "ChuwitchWindowの設定", true, None::<&str>)?;
    let restart_i = MenuItem::with_id(app, "restart", "再起動", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "終了", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&settings_i, &restart_i, &quit_i])?;

    let icon = app.default_window_icon().cloned().unwrap();

    let _tray = TrayIconBuilder::new()
        .tooltip("ウィンドウ入れ替えツール (ChuwitchWindow)")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            run_action(
                menu_action(event.id.as_ref()),
                |create| show_settings(app, create),
                || app.restart(),
                || std::process::exit(0),
            );
        })
        .on_tray_icon_event(|tray, event| {
            let action = match event {
                TrayIconEvent::Click { button, .. } => click_action(button, false),
                TrayIconEvent::DoubleClick { button, .. } => click_action(button, true),
                _ => None,
            };
            run_action(
                action,
                |create| show_settings(tray.app_handle(), create),
                || unreachable!(),
                || unreachable!(),
            );
        })
        .build(app)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn tray_input_opens_settings_and_routes_restart_and_quit() {
        let window = Cell::new(false);
        let presentations = Cell::new(0);
        let restart = Cell::new(0);
        let quit = Cell::new(0);
        let send = |action| {
            run_action(
                action,
                |create| {
                    window.set(create || window.get());
                    if window.get() {
                        presentations.set(presentations.get() + 1);
                    }
                },
                || restart.set(restart.get() + 1),
                || quit.set(quit.get() + 1),
            )
        };
        send(click_action(MouseButton::Right, true));
        send(click_action(MouseButton::Left, false));
        send(menu_action("unknown"));
        assert!(!window.get());
        send(click_action(MouseButton::Left, true));
        assert!(window.get());
        send(menu_action("settings"));
        send(click_action(MouseButton::Left, false));
        assert_eq!(presentations.get(), 3);
        send(menu_action("restart"));
        send(menu_action("quit"));
        assert_eq!((restart.get(), quit.get()), (1, 1));
    }
}
