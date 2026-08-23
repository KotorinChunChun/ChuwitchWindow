use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::thread;
use std::time::{Duration, Instant};
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

const RDP_PROCESS_NAME: &str = "mstsc.exe";
const RDP_TRANSITION_TIMEOUT: Duration = Duration::from_millis(2_000);
const RDP_POLL_INTERVAL: Duration = Duration::from_millis(40);
const RDP_RESIZE_SETTLE_DELAY: Duration = Duration::from_millis(300);

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
}

/// OS上の移動候補ウィンドウを列挙する。
/// フルスクリーンや最小化などの利用者設定は、列挙後に除外ルールで評価する。
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
    let is_fullscreen = is_fullscreen_rect(&rect.into(), style, &state.monitors);

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
    let insert_after = if hwnd_insert_after == 0 {
        HWND_TOP
    } else {
        HWND(hwnd_insert_after)
    };

    if should_reenter_rdp_fullscreen(win)
        && move_fullscreen_rdp(hwnd, dest_monitor, insert_after)
    {
        return;
    }

    let (new_x, new_y, new_w, new_h) = calculate_target_rect(win, src_monitor, dest_monitor);

    unsafe {
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

fn should_reenter_rdp_fullscreen(win: &WindowInfo) -> bool {
    win.is_fullscreen && win.process_name.eq_ignore_ascii_case(RDP_PROCESS_NAME)
}

/// mstsc の全画面表示は、ウィンドウ枠だけをリサイズしてもリモート側の
/// デスクトップ解像度が更新されない。公式ショートカットで一度ウィンドウ化し、
/// 移動先モニターへ配置してから全画面へ戻すことで、mstsc 自身に解像度を再選択させる。
fn move_fullscreen_rdp(
    hwnd: HWND,
    dest_monitor: &crate::monitor::MonitorInfo,
    insert_after: HWND,
) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, IsWindow, SetWindowPos, SWP_NOACTIVATE,
    };

    if !unsafe { IsWindow(hwnd) }.as_bool() {
        tracing::warn!("[RDP] 移動対象のウィンドウが無効です: hwnd={}", hwnd.0);
        return false;
    }

    wait_for_trigger_keys_released();
    let previous_foreground = unsafe { GetForegroundWindow() };

    if !send_rdp_fullscreen_toggle(hwnd) {
        tracing::warn!("[RDP] 全画面解除キーの送出に失敗しました: hwnd={}", hwnd.0);
        restore_foreground(previous_foreground, hwnd);
        return false;
    }

    let monitors = crate::monitor::get_all_monitors();
    if !wait_for_fullscreen_state(hwnd, false, &monitors) {
        tracing::warn!("[RDP] 全画面解除を確認できませんでした: hwnd={}", hwnd.0);
        restore_foreground(previous_foreground, hwnd);
        return false;
    }

    let (x, y, width, height) = rdp_windowed_target_rect(dest_monitor);
    let moved = unsafe {
        SetWindowPos(
            hwnd,
            insert_after,
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE,
        )
    }
    .is_ok();

    if !moved {
        tracing::warn!("[RDP] ウィンドウ化後の移動に失敗しました: hwnd={}", hwnd.0);
    }

    // WM_SIZE と動的解像度変更を処理する猶予を与えてから全画面へ戻す。
    thread::sleep(RDP_RESIZE_SETTLE_DELAY);

    let entered_fullscreen = if send_rdp_fullscreen_toggle(hwnd) {
        wait_for_fullscreen_state(hwnd, true, std::slice::from_ref(dest_monitor))
    } else {
        false
    };

    if !entered_fullscreen {
        tracing::warn!(
            "[RDP] 移動先での全画面化を確認できませんでした: hwnd={} monitor={}",
            hwnd.0,
            dest_monitor.name
        );
    } else {
        tracing::info!(
            "[RDP] 全画面解除→移動→再全画面化が完了しました: hwnd={} monitor={}",
            hwnd.0,
            dest_monitor.name
        );
    }

    restore_foreground(previous_foreground, hwnd);
    // 一度ウィンドウ化できた場合は、失敗時も通常の境界リサイズへ戻さない。
    // 再全画面化に失敗したウィンドウをさらに変形させず、ログから診断可能にする。
    true
}

fn rdp_windowed_target_rect(
    dest_monitor: &crate::monitor::MonitorInfo,
) -> (i32, i32, i32, i32) {
    let area = &dest_monitor.work_area;
    (
        area.left,
        area.top,
        area.right - area.left,
        area.bottom - area.top,
    )
}

fn send_rdp_fullscreen_toggle(hwnd: HWND) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL,
        VK_MENU, VK_PAUSE,
    };
    use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;

    let focused = unsafe { SetForegroundWindow(hwnd) }.as_bool()
        || unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() == hwnd
        };
    if !focused {
        tracing::warn!("[RDP] 対象ウィンドウを前面化できませんでした: hwnd={}", hwnd.0);
        return false;
    }
    thread::sleep(Duration::from_millis(60));

    let key = |virtual_key, key_up| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: virtual_key,
                wScan: 0,
                dwFlags: if key_up { KEYEVENTF_KEYUP } else { Default::default() },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };

    // Microsoft公式仕様で全画面／ウィンドウ表示を切り替える Ctrl+Alt+Pause。
    let inputs = [
        key(VK_CONTROL, false),
        key(VK_MENU, false),
        key(VK_PAUSE, false),
        key(VK_PAUSE, true),
        key(VK_MENU, true),
        key(VK_CONTROL, true),
    ];
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    sent == inputs.len() as u32
}

fn wait_for_trigger_keys_released() {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_CONTROL, VK_DOWN, VK_LEFT, VK_MENU, VK_RIGHT, VK_UP, VK_LWIN,
        VK_RWIN,
    };

    let keys = [
        VK_CONTROL.0 as i32,
        VK_MENU.0 as i32,
        VK_LWIN.0 as i32,
        VK_RWIN.0 as i32,
        VK_LEFT.0 as i32,
        VK_RIGHT.0 as i32,
        VK_UP.0 as i32,
        VK_DOWN.0 as i32,
    ];
    let deadline = Instant::now() + Duration::from_millis(1_000);
    while Instant::now() < deadline {
        let any_pressed = keys
            .iter()
            .any(|key| unsafe { GetAsyncKeyState(*key) } < 0);
        if !any_pressed {
            return;
        }
        thread::sleep(RDP_POLL_INTERVAL);
    }
    tracing::warn!("[RDP] 操作キーの解放待ちがタイムアウトしました");
}

fn wait_for_fullscreen_state(
    hwnd: HWND,
    expected: bool,
    monitors: &[crate::monitor::MonitorInfo],
) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongW, GetWindowRect, IsWindow, GWL_STYLE};

    let deadline = Instant::now() + RDP_TRANSITION_TIMEOUT;
    while Instant::now() < deadline {
        if !unsafe { IsWindow(hwnd) }.as_bool() {
            return false;
        }
        let mut rect = RECT::default();
        let rect_ok = unsafe { GetWindowRect(hwnd, &mut rect) }.is_ok();
        let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
        if rect_ok && is_fullscreen_rect(&rect.into(), style, monitors) == expected {
            return true;
        }
        thread::sleep(RDP_POLL_INTERVAL);
    }
    false
}

fn is_fullscreen_rect(
    rect: &MonitorRect,
    style: u32,
    monitors: &[crate::monitor::MonitorInfo],
) -> bool {
    let has_caption = (style & WS_CAPTION.0) == WS_CAPTION.0;
    let has_thickframe = (style & WS_THICKFRAME.0) == WS_THICKFRAME.0;
    monitors.iter().any(|monitor| {
        rect.left <= monitor.monitor_area.left
            && rect.top <= monitor.monitor_area.top
            && rect.right >= monitor.monitor_area.right
            && rect.bottom >= monitor.monitor_area.bottom
    }) && !has_caption
        && !has_thickframe
}

fn restore_foreground(previous: HWND, rdp: HWND) {
    use windows::Win32::UI::WindowsAndMessaging::{IsWindow, SetForegroundWindow};

    if previous.0 != 0 && previous != rdp && unsafe { IsWindow(previous) }.as_bool() {
        let _ = unsafe { SetForegroundWindow(previous) };
    }
}

fn calculate_target_rect(
    win: &WindowInfo,
    src_monitor: &crate::monitor::MonitorInfo,
    dest_monitor: &crate::monitor::MonitorInfo,
) -> (i32, i32, i32, i32) {
    // 全画面ウィンドウは作業領域の比率で拡縮すると、タスクバー差分だけ
    // 移動先モニターからはみ出す。移動先のモニター全域へ正確に合わせる。
    if win.is_fullscreen {
        let area = &dest_monitor.monitor_area;
        return (
            area.left,
            area.top,
            area.right - area.left,
            area.bottom - area.top,
        );
    }

    let window_rect = &win.rect;
    if let Some(snap) = apply_snap_correction(window_rect, &src_monitor.work_area, &dest_monitor.work_area) {
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
    }
}

pub fn move_windows(windows_to_move: &[WindowInfo], src_monitor: &crate::monitor::MonitorInfo, dest_monitor: &crate::monitor::MonitorInfo) {
    // windows_to_move is sorted top to bottom (Z-order).
    // By processing bottom to top and placing them at HWND_TOP, their relative Z-order on the new monitor is perfectly restored.
    for win in windows_to_move.iter().rev() {
        move_window(win, src_monitor, dest_monitor, 0); // 0 corresponds to HWND_TOP
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(
        name: &str,
        monitor_area: MonitorRect,
        work_area: MonitorRect,
    ) -> crate::monitor::MonitorInfo {
        crate::monitor::MonitorInfo {
            hmonitor: 0,
            name: name.to_string(),
            display_number: 1,
            manufacturer: String::new(),
            serial_number: String::new(),
            work_area,
            monitor_area,
            is_primary: false,
        }
    }

    #[test]
    fn fullscreen_window_uses_exact_destination_monitor_area() {
        let src = monitor(
            "src",
            MonitorRect { left: 3440, top: 0, right: 5360, bottom: 1080 },
            MonitorRect { left: 3440, top: 0, right: 5360, bottom: 1032 },
        );
        let dest = monitor(
            "dest",
            MonitorRect { left: 0, top: 0, right: 3440, bottom: 1440 },
            MonitorRect { left: 0, top: 0, right: 3440, bottom: 1392 },
        );
        let win = WindowInfo {
            hwnd: 1,
            title: "Remote Desktop".to_string(),
            rect: src.monitor_area.clone(),
            dpi: 96,
            is_maximized: false,
            is_minimized: false,
            is_fullscreen: true,
            process_name: "mstsc.exe".to_string(),
            class_name: "TscShellContainerClass".to_string(),
            style: 0,
            ex_style: 0,
        };

        assert_eq!(calculate_target_rect(&win, &src, &dest), (0, 0, 3440, 1440));
    }

    #[test]
    fn fullscreen_mstsc_uses_reentry_sequence_case_insensitively() {
        let win = WindowInfo {
            hwnd: 1,
            title: "Remote Desktop".to_string(),
            rect: MonitorRect { left: 0, top: 0, right: 1920, bottom: 1080 },
            dpi: 96,
            is_maximized: false,
            is_minimized: false,
            is_fullscreen: true,
            process_name: "MSTSC.EXE".to_string(),
            class_name: "TscShellContainerClass".to_string(),
            style: 0,
            ex_style: 0,
        };

        assert!(should_reenter_rdp_fullscreen(&win));
    }

    #[test]
    fn rdp_windowed_stage_uses_destination_work_area() {
        let dest = monitor(
            "dest",
            MonitorRect { left: 1920, top: 0, right: 4480, bottom: 1440 },
            MonitorRect { left: 1920, top: 0, right: 4480, bottom: 1392 },
        );

        assert_eq!(rdp_windowed_target_rect(&dest), (1920, 0, 2560, 1392));
    }
}
