use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, COINIT_MULTITHREADED, CLSCTX_INPROC_SERVER,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Shell::{IVirtualDesktopManager, VirtualDesktopManager};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetAncestor, GetLastActivePopup, GetWindowPlacement, GetWindowRect,
    GetWindowTextLengthW, GetWindowTextW, IsWindowVisible, WINDOWPLACEMENT, GA_ROOTOWNER,
    SW_SHOWMAXIMIZED, SW_SHOWMINIMIZED,
};

use crate::monitor::MonitorRect;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    pub hwnd: isize,
    pub title: String,
    pub rect: MonitorRect,
    pub dpi: u32,
    pub is_maximized: bool,
    pub is_minimized: bool,
    pub is_fullscreen: bool,
}

struct EnumState {
    windows: Vec<WindowInfo>,
    vdm: Option<IVirtualDesktopManager>,
    monitors: Vec<crate::monitor::MonitorInfo>,
}

pub fn get_target_windows() -> Vec<WindowInfo> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let vdm: Option<IVirtualDesktopManager> = unsafe {
        CoCreateInstance(&VirtualDesktopManager, None, CLSCTX_INPROC_SERVER).ok()
    };

    let mut state = Box::new(EnumState {
        windows: Vec::new(),
        vdm,
        monitors: crate::monitor::get_all_monitors(),
    });

    unsafe {
        let _ = EnumWindows(
            Some(enum_window_proc),
            LPARAM(state.as_mut() as *mut _ as isize),
        );
    }

    state.windows
}

fn is_cloaked(hwnd: HWND) -> bool {
    let mut cloaked: i32 = 0;
    let res = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut _ as *mut _,
            std::mem::size_of::<i32>() as u32,
        )
    };
    if res.is_ok() && cloaked != 0 {
        return true;
    }
    false
}

fn is_on_current_desktop(hwnd: HWND, vdm: Option<&IVirtualDesktopManager>) -> bool {
    if let Some(v) = vdm {
        let res = unsafe { v.IsWindowOnCurrentVirtualDesktop(hwnd) };
        if let Ok(is_on_current) = res {
            return is_on_current.as_bool();
        }
    }
    // APIが使えない、失敗した場合は安全のためtrue（対象に含める）とする
    true
}

fn get_window_title(hwnd: HWND) -> String {
    unsafe {
        let length = GetWindowTextLengthW(hwnd);
        if length == 0 {
            return String::new();
        }
        let mut buf = vec![0u16; (length + 1) as usize];
        GetWindowTextW(hwnd, &mut buf);
        let p = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        OsString::from_wide(&buf[..p]).to_string_lossy().into_owned()
    }
}

// EnumWindows callback
unsafe extern "system" fn enum_window_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let state = &mut *(lparam.0 as *mut EnumState);

    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }

    let title = get_window_title(hwnd);
    if title.is_empty() {
        return BOOL(1);
    }

    // Windows 10/11 app cloak filtering
    if is_cloaked(hwnd) {
        return BOOL(1);
    }

    // Virtual Desktop filtering
    if !is_on_current_desktop(hwnd, state.vdm.as_ref()) {
        return BOOL(1);
    }

    // Owner logic to filter out popups/tooltips without a proper root owner (basic heuristic)
    let mut h_root = GetAncestor(hwnd, GA_ROOTOWNER);
    if h_root.0 == 0 {
        h_root = hwnd;
    }
    if GetLastActivePopup(h_root) != hwnd {
        return BOOL(1);
    }

    let mut rect = RECT::default();
    let _ = GetWindowRect(hwnd, &mut rect);

    // Get DPI
    let dpi = GetDpiForWindow(hwnd);

    // Window placements
    let mut placement = WINDOWPLACEMENT::default();
    placement.length = std::mem::size_of::<WINDOWPLACEMENT>() as u32;
    let _ = GetWindowPlacement(hwnd, &mut placement);

    let is_maximized = placement.showCmd == SW_SHOWMAXIMIZED.0 as u32;
    let is_minimized = placement.showCmd == SW_SHOWMINIMIZED.0 as u32;

    let is_fullscreen = state.monitors.iter().any(|m| {
        rect.left == m.monitor_area.left
            && rect.top == m.monitor_area.top
            && rect.right == m.monitor_area.right
            && rect.bottom == m.monitor_area.bottom
    });

    state.windows.push(WindowInfo {
        hwnd: hwnd.0 as isize,
        title,
        rect: rect.into(),
        dpi,
        is_maximized,
        is_minimized,
        is_fullscreen,
    });

    BOOL(1)
}

fn apply_snap_correction(
    window_rect: &MonitorRect,
    src_work: &crate::monitor::MonitorRect,
    dest_work: &crate::monitor::MonitorRect,
) -> Option<(i32, i32, i32, i32)> {
    let tolerance = 15; // 隠し境界線等の誤差を吸収
    let is_approx = |a: i32, b: i32| (a - b).abs() <= tolerance;

    let src_w = src_work.right - src_work.left;
    let src_h = src_work.bottom - src_work.top;
    let dest_w = dest_work.right - dest_work.left;
    let dest_h = dest_work.bottom - dest_work.top;
    let win_w = window_rect.right - window_rect.left;
    let win_h = window_rect.bottom - window_rect.top;

    let is_full_h = is_approx(window_rect.top, src_work.top) && is_approx(win_h, src_h);
    let is_half_w = is_approx(win_w, src_w / 2);

    // Left Snap
    if is_approx(window_rect.left, src_work.left) && is_half_w && is_full_h {
        return Some((dest_work.left, dest_work.top, dest_w / 2, dest_h));
    }
    // Right Snap
    if is_approx(window_rect.right, src_work.right) && is_half_w && is_full_h {
        return Some((dest_work.right - dest_w / 2, dest_work.top, dest_w / 2, dest_h));
    }
    // Top-Left Quarter
    if is_approx(window_rect.left, src_work.left) && is_approx(window_rect.top, src_work.top) && is_approx(win_w, src_w / 2) && is_approx(win_h, src_h / 2) {
        return Some((dest_work.left, dest_work.top, dest_w / 2, dest_h / 2));
    }
    // Top-Right Quarter
    if is_approx(window_rect.right, src_work.right) && is_approx(window_rect.top, src_work.top) && is_approx(win_w, src_w / 2) && is_approx(win_h, src_h / 2) {
        return Some((dest_work.right - dest_w / 2, dest_work.top, dest_w / 2, dest_h / 2));
    }
    // Bottom-Left Quarter
    if is_approx(window_rect.left, src_work.left) && is_approx(window_rect.bottom, src_work.bottom) && is_approx(win_w, src_w / 2) && is_approx(win_h, src_h / 2) {
        return Some((dest_work.left, dest_work.bottom - dest_h / 2, dest_w / 2, dest_h / 2));
    }
    // Bottom-Right Quarter
    if is_approx(window_rect.right, src_work.right) && is_approx(window_rect.bottom, src_work.bottom) && is_approx(win_w, src_w / 2) && is_approx(win_h, src_h / 2) {
        return Some((dest_work.right - dest_w / 2, dest_work.bottom - dest_h / 2, dest_w / 2, dest_h / 2));
    }

    None
}

pub fn move_window(
    hwnd: isize,
    window_rect: &MonitorRect,
    src_monitor: &crate::monitor::MonitorInfo,
    dest_monitor: &crate::monitor::MonitorInfo,
    hwnd_insert_after: isize,
) {
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, HWND_TOP};

    let hwnd = HWND(hwnd);
    
    let (new_x, new_y, new_w, new_h) = if let Some(snap) = apply_snap_correction(window_rect, &src_monitor.work_area, &dest_monitor.work_area) {
        snap
    } else {
        // Fallback to ratio scaling
        let src_w = src_monitor.work_area.right - src_monitor.work_area.left;
        let src_h = src_monitor.work_area.bottom - src_monitor.work_area.top;
        let win_w = window_rect.right - window_rect.left;
        let win_h = window_rect.bottom - window_rect.top;
        
        let ratio_x = (dest_monitor.work_area.right - dest_monitor.work_area.left) as f32 / src_w as f32;
        let ratio_y = (dest_monitor.work_area.bottom - dest_monitor.work_area.top) as f32 / src_h as f32;
        
        let rel_x = window_rect.left - src_monitor.work_area.left;
        let rel_y = window_rect.top - src_monitor.work_area.top;
        
        (
            dest_monitor.work_area.left + (rel_x as f32 * ratio_x).round() as i32,
            dest_monitor.work_area.top + (rel_y as f32 * ratio_y).round() as i32,
            (win_w as f32 * ratio_x).round() as i32,
            (win_h as f32 * ratio_y).round() as i32
        )
    };

    unsafe {
        let insert_after = if hwnd_insert_after == 0 {
            HWND_TOP
        } else {
            HWND(hwnd_insert_after)
        };
        
        let _ = SetWindowPos(
            hwnd,
            insert_after,
            new_x,
            new_y,
            new_w,
            new_h,
            SWP_NOACTIVATE,
        );
    }
}

pub fn move_windows(windows_to_move: &[WindowInfo], src_monitor: &crate::monitor::MonitorInfo, dest_monitor: &crate::monitor::MonitorInfo) {
    // windows_to_move is sorted top to bottom (Z-order).
    // By processing bottom to top and placing them at HWND_TOP, their relative Z-order on the new monitor is perfectly restored.
    for win in windows_to_move.iter().rev() {
        move_window(win.hwnd, &win.rect, src_monitor, dest_monitor, 0); // 0 corresponds to HWND_TOP
    }
}
