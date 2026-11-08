use crate::monitor::{MonitorInfo, MonitorRect};
use crate::window::WindowInfo;
use windows::core::w;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, HMENU, WM_MBUTTONDOWN, WM_MBUTTONUP,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

pub fn rect(l: i32, t: i32, r: i32, b: i32) -> MonitorRect {
    assert!(r > l && b > t, "invalid rect");
    MonitorRect {
        left: l,
        top: t,
        right: r,
        bottom: b,
    }
}

pub struct WindowBuilder {
    hwnd: isize,
    title: String,
    process: String,
    class: String,
    rect: MonitorRect,
    fullscreen: bool,
    minimized: bool,
    maximized: bool,
    style: u32,
    ex_style: u32,
    dpi: u32,
}

impl WindowBuilder {
    pub fn new(hwnd: isize) -> Self {
        Self {
            hwnd,
            title: "Untitled".to_string(),
            process: "notepad.exe".to_string(),
            class: "Notepad".to_string(),
            rect: rect(0, 0, 800, 600),
            fullscreen: false,
            minimized: false,
            maximized: false,
            style: 0,
            ex_style: 0,
            dpi: 96,
        }
    }

    pub fn title(mut self, v: &str) -> Self {
        self.title = v.to_string();
        self
    }

    pub fn process(mut self, v: &str) -> Self {
        self.process = v.to_string();
        self
    }

    pub fn class(mut self, v: &str) -> Self {
        self.class = v.to_string();
        self
    }

    pub fn rect(mut self, l: i32, t: i32, r: i32, b: i32) -> Self {
        self.rect = rect(l, t, r, b);
        self
    }

    pub fn fullscreen(mut self, v: bool) -> Self {
        self.fullscreen = v;
        self
    }

    pub fn minimized(mut self, v: bool) -> Self {
        self.minimized = v;
        self
    }

    pub fn maximized(mut self, v: bool) -> Self {
        self.maximized = v;
        self
    }

    pub fn style(mut self, v: u32) -> Self {
        self.style = v;
        self
    }

    pub fn build(self) -> WindowInfo {
        WindowInfo {
            hwnd: self.hwnd,
            title: self.title,
            rect: self.rect,
            dpi: self.dpi,
            is_maximized: self.maximized,
            is_minimized: self.minimized,
            is_fullscreen: self.fullscreen,
            process_name: self.process,
            class_name: self.class,
            style: self.style,
            ex_style: self.ex_style,
        }
    }
}

pub fn monitor_1920(name: &str, origin_x: i32) -> MonitorInfo {
    let area = rect(origin_x, 0, origin_x + 1920, 1080);
    MonitorInfo {
        hmonitor: origin_x as isize,
        name: name.to_string(),
        display_number: if origin_x == 0 { 1 } else { 2 },
        manufacturer: String::new(),
        serial_number: String::new(),
        work_area: area.clone(),
        monitor_area: area,
        is_primary: origin_x == 0,
    }
}

pub fn dual_setup() -> Vec<MonitorInfo> {
    vec![monitor_1920("M1", 0), monitor_1920("M2", 1920)]
}

pub(crate) struct TestWindow(pub HWND);

thread_local! {
    pub(crate) static MIDDLE_EVENTS: std::cell::RefCell<Vec<u32>> = const { std::cell::RefCell::new(Vec::new()) };
}

impl Drop for TestWindow {
    fn drop(&mut self) {
        unsafe { DestroyWindow(self.0).unwrap() };
    }
}

pub(crate) fn test_window(
    style: windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE,
    x: i32,
    y: i32,
) -> TestWindow {
    use std::sync::Once;
    use windows::Win32::UI::WindowsAndMessaging::{RegisterClassW, WNDCLASSW};

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if matches!(message, WM_MBUTTONDOWN | WM_MBUTTONUP) {
            MIDDLE_EVENTS.with_borrow_mut(|events| events.push(message));
        }
        DefWindowProcW(hwnd, message, wparam, lparam)
    }

    static REGISTER: Once = Once::new();
    let name = w!("ChuwitchWindowDragTest");
    let instance = HINSTANCE(unsafe { GetModuleHandleW(None).unwrap() }.0);
    REGISTER.call_once(|| {
        assert_ne!(
            unsafe {
                RegisterClassW(&WNDCLASSW {
                    lpfnWndProc: Some(window_proc),
                    hInstance: instance,
                    lpszClassName: name,
                    ..Default::default()
                })
            },
            0
        );
    });
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            name,
            name,
            style,
            x,
            y,
            400,
            260,
            HWND::default(),
            HMENU::default(),
            instance,
            None,
        )
    };
    assert_ne!(hwnd.0, 0);
    TestWindow(hwnd)
}
