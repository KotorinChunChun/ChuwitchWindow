use std::sync::OnceLock;
use tauri::{AppHandle, Emitter};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW,
    TranslateMessage, WM_DISPLAYCHANGE, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};

static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_DISPLAYCHANGE {
        tracing::info!("WM_DISPLAYCHANGE detected!");
        if let Some(app) = APP_HANDLE.get() {
            let _ = app.emit("display-changed", ());
        }
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

pub fn start_hotplug_listener(app_handle: AppHandle) {
    if APP_HANDLE.set(app_handle).is_err() {
        tracing::warn!("Hotplug listener already started");
        return;
    }

    std::thread::spawn(move || {
        unsafe {
            let class_name = windows::core::w!("ChuwitchHotplugClass");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wnd_proc),
                lpszClassName: class_name,
                ..Default::default()
            };
            let _ = RegisterClassW(&wc);

            let _hwnd = CreateWindowExW(
                windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE::default(),
                class_name,
                windows::core::w!("ChuwitchHotplug"),
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                0,
                0,
                HWND::default(),
                None,
                None,
                None,
            );

            let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
            while GetMessageW(&mut msg, HWND::default(), 0, 0).into() {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    });
}
