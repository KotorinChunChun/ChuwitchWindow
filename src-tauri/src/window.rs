use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT, CloseHandle};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, COINIT_MULTITHREADED, CLSCTX_INPROC_SERVER,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
    GetCurrentProcessId,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Shell::{IVirtualDesktopManager, VirtualDesktopManager};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetAncestor, GetClassNameW, GetLastActivePopup, GetWindowPlacement,
    GetWindowRect, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
    IsWindowVisible, WINDOWPLACEMENT, GA_ROOTOWNER,
    SW_SHOWMAXIMIZED, SW_SHOWMINIMIZED, GetWindowLongW, GWL_STYLE, GWL_EXSTYLE, WS_CAPTION, WS_THICKFRAME,
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
    /// プロセス名（例: "notepad.exe"）― 除外ルール用 (v0.2)
    pub process_name: String,
    /// ウィンドウクラス名（例: "Notepad"）― 除外ルール用 (v0.2)
    pub class_name: String,
    pub style: u32,
    pub ex_style: u32,
}

struct EnumState {
    windows: Vec<WindowInfo>,
    vdm: Option<IVirtualDesktopManager>,
    monitors: Vec<crate::monitor::MonitorInfo>,
    ignore_fullscreen: bool,
    /// v0.2: 最小化ウィンドウを移動対象から除外するか
    exclude_minimized: bool,
}

pub fn get_target_windows(ignore_fullscreen: bool, exclude_minimized: bool) -> Vec<WindowInfo> {
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
        ignore_fullscreen,
        exclude_minimized,
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

    // 自プロセスのウィンドウは除外（設定画面やポップアップなど）
    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if unsafe { GetCurrentProcessId() } == pid {
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

    // DPI を取得
    let dpi = GetDpiForWindow(hwnd);

    // ウィンドウ配置情報を取得
    let mut placement = WINDOWPLACEMENT::default();
    placement.length = std::mem::size_of::<WINDOWPLACEMENT>() as u32;
    let _ = GetWindowPlacement(hwnd, &mut placement);

    let is_maximized = placement.showCmd == SW_SHOWMAXIMIZED.0 as u32;
    let is_minimized = placement.showCmd == SW_SHOWMINIMIZED.0 as u32;

    let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
    let ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;
    let has_caption = (style & WS_CAPTION.0) == WS_CAPTION.0;
    let has_thickframe = (style & WS_THICKFRAME.0) == WS_THICKFRAME.0;

    let is_fullscreen = state.monitors.iter().any(|m| {
        // ウィンドウの領域がモニター全画面領域と等しい、またはそれより大きい
        rect.left <= m.monitor_area.left
            && rect.top <= m.monitor_area.top
            && rect.right >= m.monitor_area.right
            && rect.bottom >= m.monitor_area.bottom
    }) && (!has_caption && !has_thickframe);

    if state.ignore_fullscreen && is_fullscreen {
        return BOOL(1);
    }

    // v0.2: 最小化ウィンドウの除外 (SPEC 2-5)
    if state.exclude_minimized && is_minimized {
        return BOOL(1);
    }

    // --- v0.2: プロセス名を取得 ---
    let process_name = {
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid != 0 {
            if let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                let mut buf = [0u16; 260];
                let mut len = buf.len() as u32;
                if QueryFullProcessImageNameW(process, PROCESS_NAME_FORMAT(0), windows::core::PWSTR(buf.as_mut_ptr()), &mut len).is_ok() {
                    let full_path = OsString::from_wide(&buf[..len as usize]).to_string_lossy().into_owned();
                    let _ = CloseHandle(process);
                    // フルパスからファイル名部分のみ抽出
                    full_path.rsplit('\\').next().unwrap_or(&full_path).to_string()
                } else {
                    let _ = CloseHandle(process);
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    };

    // --- v0.2: ウィンドウクラス名を取得 ---
    let class_name = {
        let mut buf = [0u16; 256];
        let len = GetClassNameW(hwnd, &mut buf);
        if len > 0 {
            OsString::from_wide(&buf[..len as usize]).to_string_lossy().into_owned()
        } else {
            String::new()
        }
    };

    state.windows.push(WindowInfo {
        hwnd: hwnd.0 as isize,
        title,
        rect: rect.into(),
        dpi,
        is_maximized,
        is_minimized,
        is_fullscreen,
        process_name,
        class_name,
        style,
        ex_style,
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
    win: &WindowInfo,
    src_monitor: &crate::monitor::MonitorInfo,
    dest_monitor: &crate::monitor::MonitorInfo,
    hwnd_insert_after: isize,
) {
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, ShowWindow, SWP_NOACTIVATE, HWND_TOP, SW_RESTORE, SW_MAXIMIZE};

    let hwnd = HWND(win.hwnd);
    let window_rect = &win.rect;
    
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
        
        if win.is_maximized {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }

        let _ = SetWindowPos(
            hwnd,
            insert_after,
            new_x,
            new_y,
            new_w,
            new_h,
            SWP_NOACTIVATE,
        );

        if win.is_maximized {
            let _ = ShowWindow(hwnd, SW_MAXIMIZE);
        }
    }
}

pub fn move_windows(windows_to_move: &[WindowInfo], src_monitor: &crate::monitor::MonitorInfo, dest_monitor: &crate::monitor::MonitorInfo) {
    // windows_to_move is sorted top to bottom (Z-order).
    // By processing bottom to top and placing them at HWND_TOP, their relative Z-order on the new monitor is perfectly restored.
    for win in windows_to_move.iter().rev() {
        move_window(win, src_monitor, dest_monitor, 0); // 0 corresponds to HWND_TOP
    }
}
