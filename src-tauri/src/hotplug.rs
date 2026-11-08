use std::cell::RefCell;
use std::sync::Once;
use tauri::{AppHandle, Emitter};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW,
    TranslateMessage, WM_DISPLAYCHANGE, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};

thread_local! {
    static NOTIFY: RefCell<Option<Box<dyn Fn(&str)>>> = RefCell::new(None);
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_DISPLAYCHANGE {
        tracing::info!("WM_DISPLAYCHANGE detected!");
        NOTIFY.with_borrow(|notify| {
            if let Some(notify) = notify {
                notify("display-changed");
            }
        });
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

pub fn start_hotplug_listener(app_handle: AppHandle) {
    static START: Once = Once::new();
    START.call_once(move || {
        std::thread::spawn(move || unsafe {
            let hwnd = listener_window(move |event| {
                let _ = app_handle.emit(event, ());
            });
            if hwnd.0 == 0 {
                tracing::warn!("ディスプレイ変更の通知ウィンドウを作成できません");
                return;
            }
            let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
            while GetMessageW(&mut msg, HWND::default(), 0, 0).0 > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        });
    });
}

fn listener_window(notify: impl Fn(&str) + 'static) -> HWND {
    use windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE;
    static REGISTER: Once = Once::new();
    let class_name = windows::core::w!("ChuwitchHotplugClass");
    NOTIFY.with_borrow_mut(|handler| *handler = Some(Box::new(notify)));
    unsafe {
        REGISTER.call_once(|| {
            let _ = RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(wnd_proc),
                lpszClassName: class_name,
                ..Default::default()
            });
        });
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
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
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use windows::Win32::UI::WindowsAndMessaging::{DestroyWindow, SendMessageW, WM_NULL};

    #[test]
    fn display_change_message_notifies_the_application_without_changing_monitors() {
        let count = Arc::new(AtomicUsize::new(0));
        let listener_count = count.clone();
        let hwnd = listener_window(move |event| {
            assert_eq!(event, "display-changed");
            listener_count.fetch_add(1, Ordering::Relaxed);
        });
        assert_ne!(hwnd.0, 0);
        unsafe { SendMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0)) };
        assert_eq!(count.load(Ordering::Relaxed), 0);
        unsafe { SendMessageW(hwnd, WM_DISPLAYCHANGE, WPARAM(32), LPARAM(0)) };
        unsafe { DestroyWindow(hwnd) }.unwrap();
        NOTIFY.with_borrow_mut(Option::take);
        assert_eq!(count.load(Ordering::Relaxed), 1);
    }
}
