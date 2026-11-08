use crate::config::AppConfig;
use crate::monitor::MonitorRect;
use crate::rule::{is_excluded, parse_rules, RuleNode};
use crate::{pin, window};
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::{MonitorFromWindow, MONITOR_DEFAULTTONULL};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetAncestor, GetMessageW, GetShellWindow, GetWindowRect, GetWindowTextLengthW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, IsZoomed, SetWindowPos, SetWindowsHookExW,
    UnhookWindowsHookEx, WindowFromPoint, GA_ROOT, HHOOK, LLMHF_INJECTED, MSG, MSLLHOOKSTRUCT,
    SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, WH_MOUSE_LL, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEMOVE,
};

const SNAP_DISTANCE: i32 = 12;
const DRAG_DISTANCE: i32 = 4;
static ENABLED: AtomicBool = AtomicBool::new(false);
static OPTIONS: Mutex<Option<(bool, RuleNode)>> = Mutex::new(None);

struct Gesture {
    target: isize,
    origin: POINT,
    latest: Mutex<(POINT, bool)>,
}

thread_local! {
    static CAPTURE: RefCell<Option<Arc<Gesture>>> = const { RefCell::new(None) };
    static START: RefCell<Option<SyncSender<Arc<Gesture>>>> = const { RefCell::new(None) };
}

pub fn configure(config: &AppConfig) {
    *OPTIONS.lock().unwrap() = Some((
        config.alt_drag_follow_neighbors,
        parse_rules(&config.exclusion_rules),
    ));
    ENABLED.store(config.alt_drag_enabled, Ordering::Release);
}

pub fn init(config: &AppConfig) {
    configure(config);
    let (send, receive) = mpsc::sync_channel::<Arc<Gesture>>(1);
    thread::spawn(move || {
        // フックとは別のスレッドで、列挙・吸着判定・移動を行う。
        unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        while let Ok(gesture) = receive.recv() {
            run_gesture(&gesture);
        }
    });
    thread::spawn(move || unsafe {
        START.with(|slot| slot.replace(Some(send)));
        SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let hook = match install_hook() {
            Ok(hook) => hook,
            Err(error) => {
                tracing::warn!("Alt＋左ドラッグを開始できません: {error}");
                return;
            }
        };
        let mut message = MSG::default();
        while GetMessageW(&mut message, HWND::default(), 0, 0).0 > 0 {}
        let _ = UnhookWindowsHookEx(hook);
    });
}

fn install_hook() -> windows::core::Result<HHOOK> {
    unsafe {
        let module = GetModuleHandleW(None)?;
        SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), HINSTANCE(module.0), 0)
    }
}

unsafe extern "system" fn mouse_hook(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
    if code >= 0 {
        let event = &*(data.0 as *const MSLLHOOKSTRUCT);
        if event.flags & LLMHF_INJECTED == 0 && record_mouse(message.0 as u32, event.pt) {
            return LRESULT(1);
        }
    }
    CallNextHookEx(HHOOK::default(), code, message, data)
}

fn record_mouse(message: u32, point: POINT) -> bool {
    CAPTURE.with(|slot| {
        let mut capture = slot.borrow_mut();
        if matches!(message, WM_MOUSEMOVE | WM_LBUTTONUP) {
            if let Some(gesture) = capture.as_ref() {
                let finished = message == WM_LBUTTONUP;
                *gesture.latest.lock().unwrap() = (point, finished);
                if finished {
                    *capture = None;
                }
                return finished;
            }
        }
        if message != WM_LBUTTONDOWN {
            return false;
        }
        if let Some(previous) = capture.take() {
            previous.latest.lock().unwrap().1 = true;
        }
        if !ENABLED.load(Ordering::Acquire) || !alt_only() {
            return false;
        }
        let hwnd = unsafe { GetAncestor(WindowFromPoint(point), GA_ROOT) };
        if hwnd.0 == 0
            || hwnd == unsafe { GetShellWindow() }
            || !normal_window(hwnd)
            || window_pid(hwnd) == std::process::id()
            || unsafe { GetWindowTextLengthW(hwnd) } == 0
            || pin::is_pinned(hwnd.0)
        {
            return false;
        }
        let gesture = Arc::new(Gesture {
            target: hwnd.0,
            origin: point,
            latest: Mutex::new((point, false)),
        });
        if START.with(|sender| {
            sender
                .borrow()
                .as_ref()
                .is_some_and(|send| send.try_send(gesture.clone()).is_ok())
        }) {
            *capture = Some(gesture);
            true
        } else {
            false
        }
    })
}

fn alt_only() -> bool {
    let held = |key: u16| unsafe { GetAsyncKeyState(key.into()) } < 0;
    held(VK_MENU.0)
        && ![VK_CONTROL, VK_SHIFT, VK_LWIN, VK_RWIN]
            .iter()
            .any(|key| held(key.0))
}

fn normal_window(hwnd: HWND) -> bool {
    unsafe {
        IsWindowVisible(hwnd).as_bool() && !IsIconic(hwnd).as_bool() && !IsZoomed(hwnd).as_bool()
    }
}

fn window_pid(hwnd: HWND) -> u32 {
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    pid
}

type Offset = (i32, i32);

struct DragWindow {
    hwnd: HWND,
    pid: u32,
    raw: MonitorRect,
    frame: MonitorRect,
}

impl DragWindow {
    fn new(hwnd: HWND) -> Option<Self> {
        let pid = window_pid(hwnd);
        let mut raw = RECT::default();
        if pid == 0 || unsafe { GetWindowRect(hwnd, &mut raw) }.is_err() {
            return None;
        }
        let mut frame = raw;
        let visible_frame = unsafe {
            DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                (&mut frame as *mut RECT).cast(),
                std::mem::size_of::<RECT>() as u32,
            )
        };
        if visible_frame.is_err() || frame.left >= frame.right || frame.top >= frame.bottom {
            frame = raw;
        }
        Some(Self {
            hwnd,
            pid,
            raw: raw.into(),
            frame: frame.into(),
        })
    }

    fn move_by(&self, (x, y): Offset) -> bool {
        normal_window(self.hwnd)
            && window_pid(self.hwnd) == self.pid
            && unsafe {
                SetWindowPos(
                    self.hwnd,
                    HWND::default(),
                    self.raw.left + x,
                    self.raw.top + y,
                    0,
                    0,
                    SWP_ASYNCWINDOWPOS | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                )
            }
            .is_ok()
    }
}

fn run_gesture(gesture: &Gesture) {
    let Some((follow, rules)) = OPTIONS.lock().unwrap().clone() else {
        return;
    };
    let target = HWND(gesture.target);
    let source_monitor = unsafe { MonitorFromWindow(target, MONITOR_DEFAULTTONULL) };
    let mut windows = window::get_target_windows()
        .into_iter()
        .filter(|w| !w.is_minimized && !w.is_maximized && !w.is_fullscreen)
        .filter(|w| !pin::is_pinned(w.hwnd) && !is_excluded(w, &rules))
        .filter(|w| {
            w.hwnd == gesture.target
                || follow
                    && source_monitor.0 != 0
                    && unsafe { MonitorFromWindow(HWND(w.hwnd), MONITOR_DEFAULTTONULL) }
                        == source_monitor
        })
        .filter_map(|w| DragWindow::new(HWND(w.hwnd)))
        .collect::<Vec<_>>();
    let Some(index) = windows.iter().position(|w| w.hwnd == target) else {
        return;
    };
    let lead = windows.swap_remove(index);
    let mut followers = Vec::new();
    track_gesture(gesture, &ENABLED, |offset, first_move| {
        if !lead.move_by(offset) {
            return false;
        }
        if first_move {
            attach(&lead.frame, (0, 0), &mut windows, &mut followers);
        }
        attach(&lead.frame, offset, &mut windows, &mut followers);
        followers
            .retain(|(window, anchor)| window.move_by((offset.0 + anchor.0, offset.1 + anchor.1)));
        true
    });
}

fn track_gesture(
    gesture: &Gesture,
    enabled: &AtomicBool,
    mut move_to: impl FnMut(Offset, bool) -> bool,
) {
    let mut last = (0, 0);
    let mut dragging = false;
    // フックが押下を消費するため、ボタンの状態はフックで受け取ったイベントで追跡する。
    while enabled.load(Ordering::Acquire) {
        let (point, finished) = *gesture.latest.lock().unwrap();
        let offset = (point.x - gesture.origin.x, point.y - gesture.origin.y);
        if offset != last && (dragging || offset.0.abs().max(offset.1.abs()) >= DRAG_DISTANCE) {
            if !move_to(offset, !dragging) {
                return;
            }
            dragging = true;
            last = offset;
        }
        if finished {
            break;
        }
        thread::sleep(Duration::from_millis(16));
    }
}

fn attach(
    lead: &MonitorRect,
    (x, y): Offset,
    candidates: &mut Vec<DragWindow>,
    followers: &mut Vec<(DragWindow, Offset)>,
) {
    let moved = MonitorRect {
        left: lead.left + x,
        right: lead.right + x,
        top: lead.top + y,
        bottom: lead.bottom + y,
    };
    let mut index = 0;
    while index < candidates.len() {
        if let Some((snap_x, snap_y)) = snap_offset(&moved, &candidates[index].frame) {
            followers.push((candidates.swap_remove(index), (snap_x - x, snap_y - y)));
        } else {
            index += 1;
        }
    }
}

fn snap_offset(lead: &MonitorRect, other: &MonitorRect) -> Option<Offset> {
    if lead.top < other.bottom && other.top < lead.bottom {
        for dx in [lead.right - other.left, lead.left - other.right] {
            if dx.abs() <= SNAP_DISTANCE {
                return Some((dx, 0));
            }
        }
    }
    if lead.left < other.right && other.left < lead.right {
        for dy in [lead.bottom - other.top, lead.top - other.bottom] {
            if dy.abs() <= SNAP_DISTANCE {
                return Some((0, dy));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> MonitorRect {
        MonitorRect {
            left,
            top,
            right,
            bottom,
        }
    }

    #[test]
    fn drag_is_opt_in_and_windows_accepts_the_mouse_hook() {
        let config: AppConfig = serde_json::from_str("{}").unwrap();
        assert!(!config.alt_drag_enabled && !config.alt_drag_follow_neighbors);
        let hook = install_hook().expect("Windowsのマウスフックを登録");
        unsafe { UnhookWindowsHookEx(hook) }.expect("試験フックを解除");
    }

    #[test]
    fn captured_drag_processes_multiple_moves_and_the_release_position() {
        let gesture = Arc::new(Gesture {
            target: 0,
            origin: POINT { x: 0, y: 0 },
            latest: Mutex::new((POINT { x: 0, y: 0 }, false)),
        });
        CAPTURE.with(|slot| slot.replace(Some(gesture.clone())));
        assert!(!record_mouse(WM_MOUSEMOVE, POINT { x: 10, y: 5 }));

        let enabled = AtomicBool::new(true);
        let mut moves = Vec::new();
        track_gesture(&gesture, &enabled, |offset, first_move| {
            moves.push((offset, first_move));
            match moves.len() {
                1 => assert!(!record_mouse(WM_MOUSEMOVE, POINT { x: 20, y: 15 })),
                2 => assert!(record_mouse(WM_LBUTTONUP, POINT { x: 25, y: 20 })),
                _ => {}
            }
            true
        });

        assert_eq!(
            moves,
            [((10, 5), true), ((20, 15), false), ((25, 20), false)]
        );
        assert!(!record_mouse(WM_MOUSEMOVE, POINT { x: 30, y: 30 }));
    }

    #[test]
    fn only_nearby_edges_with_overlapping_spans_snap() {
        let lead = rect(0, 0, 100, 100);
        assert_eq!(snap_offset(&lead, &rect(112, 20, 190, 120)), Some((-12, 0)));
        assert_eq!(snap_offset(&lead, &rect(-80, 0, -5, 100)), Some((5, 0)));
        assert_eq!(snap_offset(&lead, &rect(25, 105, 150, 195)), Some((0, -5)));
        assert_eq!(snap_offset(&lead, &rect(0, -80, 75, -2)), Some((0, 2)));
        assert_eq!(snap_offset(&lead, &rect(113, 20, 190, 120)), None);
        assert_eq!(snap_offset(&lead, &rect(100, 100, 175, 175)), None);
        assert_eq!(snap_offset(&lead, &rect(0, 0, 100, 100)), None);
    }

    #[test]
    fn a_later_attachment_preserves_the_relative_position() {
        let lead = rect(-300, 0, -200, 100);
        let frame = rect(-150, 10, -50, 110);
        let mut candidates = vec![DragWindow {
            hwnd: HWND::default(),
            pid: 0,
            raw: frame.clone(),
            frame,
        }];
        let mut followers = Vec::new();
        attach(&lead, (20, 0), &mut candidates, &mut followers);
        assert_eq!(candidates.len(), 1);
        attach(&lead, (45, 0), &mut candidates, &mut followers);
        assert_eq!(followers[0].1, (-50, 0));
        attach(&lead, (70, 20), &mut candidates, &mut followers);
        let (window, anchor) = &followers[0];
        assert_eq!(
            (
                window.raw.left + 70 + anchor.0,
                window.raw.top + 20 + anchor.1
            ),
            (-130, 30)
        );
        assert!(candidates.is_empty());
    }
}
