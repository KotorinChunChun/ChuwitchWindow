use crate::monitor::{MonitorInfo, MonitorRect};
use crate::window::WindowInfo;

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
