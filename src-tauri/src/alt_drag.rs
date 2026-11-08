mod layout;
mod visibility;

use crate::config::AppConfig;
use crate::monitor::MonitorRect;
use crate::rule::{is_excluded, parse_rules, RuleNode};
use crate::{pin, window};
use layout::{DragLayout, MonitorArea, Placement};
use std::cell::RefCell;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, HMONITOR, MONITORINFO,
    MONITOR_DEFAULTTONULL,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetAncestor, GetMessageW, GetShellWindow, GetWindowRect, GetWindowTextLengthW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, IsZoomed, SendMessageTimeoutW,
    SetWindowPos, SetWindowsHookExW, UnhookWindowsHookEx, WindowFromPoint, GA_ROOT, HHOOK,
    HTCAPTION, HWND_TOP, LLMHF_INJECTED, MSG, MSLLHOOKSTRUCT, SMTO_ABORTIFHUNG, SMTO_BLOCK,
    SMTO_ERRORONEXIT, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER,
    SWP_NOSIZE, SWP_NOZORDER, WH_MOUSE_LL, WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_MBUTTONUP,
    WM_MOUSEMOVE, WM_NCHITTEST,
};

const SNAP_DISTANCE: i32 = 12;
const DRAG_DISTANCE: i32 = 4;
const TITLE_HIT_TIMEOUT_MS: u32 = 16;
const MAX_GROUP_WINDOWS: usize = 8;
const MIDDLE_DRAG: u8 = 1;
const FOLLOW_NEIGHBORS: u8 = 2;
static ENABLED: AtomicU8 = AtomicU8::new(0);
static OPTIONS: Mutex<Option<DragOptions>> = Mutex::new(None);

#[derive(Clone)]
struct DragOptions {
    follow: bool,
    snap: bool,
    resize: bool,
    rules: RuleNode,
}

struct Gesture {
    target: WindowKey,
    origin: POINT,
    latest: Mutex<(POINT, bool)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WindowKey {
    hwnd: isize,
    pid: u32,
}

impl WindowKey {
    fn is_current(self) -> bool {
        self.pid != 0 && window_pid(HWND(self.hwnd)) == self.pid
    }
}

enum Work {
    Drag(Arc<Gesture>),
    TitleClick { target: WindowKey, point: POINT },
}

thread_local! {
    static CAPTURE: RefCell<Option<Arc<Gesture>>> = const { RefCell::new(None) };
    static WORKER: RefCell<Option<SyncSender<Work>>> = const { RefCell::new(None) };
}

pub fn configure(config: &AppConfig) {
    *OPTIONS.lock().unwrap() = Some(DragOptions {
        follow: config.alt_drag_follow_neighbors,
        snap: config.drag_snap_to_work_area,
        resize: config.drag_resize_across_monitors,
        rules: parse_rules(&config.exclusion_rules),
    });
    let mut enabled = 0;
    if config.middle_drag_enabled {
        enabled |= MIDDLE_DRAG;
        if config.alt_drag_follow_neighbors {
            enabled |= FOLLOW_NEIGHBORS;
        }
    }
    ENABLED.store(enabled, Ordering::Release);
}

pub fn init(config: &AppConfig) {
    configure(config);
    let (send, receive) = mpsc::sync_channel::<Work>(1);
    thread::spawn(move || {
        // フックとは別のスレッドで、列挙・吸着判定・移動を行う。
        unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        let mut groups = Vec::new();
        while let Ok(work) = receive.recv() {
            groups.retain_mut(|group: &mut Vec<WindowKey>| {
                group.retain(|key| key.is_current());
                group.len() > 1
            });
            match work {
                Work::Drag(gesture) => {
                    if ENABLED.load(Ordering::Acquire) & FOLLOW_NEIGHBORS != 0 {
                        raise_group(&groups, gesture.target);
                    }
                    run_gesture(&gesture, &mut groups);
                }
                Work::TitleClick { target, point } => {
                    if ENABLED.load(Ordering::Acquire) & FOLLOW_NEIGHBORS != 0
                        && groups.iter().any(|group| group.contains(&target))
                        && is_title_bar(HWND(target.hwnd), point, 75)
                    {
                        raise_group(&groups, target);
                    }
                }
            }
        }
    });
    thread::spawn(move || unsafe {
        WORKER.with(|slot| slot.replace(Some(send)));
        SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let hook = match install_hook() {
            Ok(hook) => hook,
            Err(error) => {
                tracing::warn!("ウィンドウのドラッグ移動を開始できません: {error}");
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
        if let Some(gesture) = capture.as_ref() {
            let finished = message == WM_MBUTTONUP;
            if message == WM_MOUSEMOVE || finished {
                *gesture.latest.lock().unwrap() = (point, finished);
                if finished {
                    *capture = None;
                }
                return finished;
            }
            if message != WM_MBUTTONDOWN {
                return false;
            }
            gesture.latest.lock().unwrap().1 = true;
            *capture = None;
        }
        if !matches!(message, WM_LBUTTONDOWN | WM_MBUTTONDOWN) {
            return false;
        }
        let enabled = ENABLED.load(Ordering::Acquire);
        if message == WM_LBUTTONDOWN && enabled & FOLLOW_NEIGHBORS != 0 && unmodified() {
            let hwnd = unsafe { GetAncestor(WindowFromPoint(point), GA_ROOT) };
            let pid = window_pid(hwnd);
            if pid != 0 && pid != std::process::id() {
                send_work(Work::TitleClick {
                    target: WindowKey { hwnd: hwnd.0, pid },
                    point,
                });
            }
            return false;
        }
        if message != WM_MBUTTONDOWN || enabled & MIDDLE_DRAG == 0 || !unmodified() {
            return false;
        }
        let hwnd = unsafe { WindowFromPoint(point) };
        let pid = window_pid(hwnd);
        if hwnd.0 == 0
            || hwnd != unsafe { GetAncestor(hwnd, GA_ROOT) }
            || hwnd == unsafe { GetShellWindow() }
            || !normal_window(hwnd)
            || pid == 0
            || pid == std::process::id()
            || unsafe { GetWindowTextLengthW(hwnd) } == 0
            || pin::is_pinned(hwnd.0)
            || !is_title_bar(hwnd, point, TITLE_HIT_TIMEOUT_MS)
        {
            return false;
        }
        let gesture = Arc::new(Gesture {
            target: WindowKey { hwnd: hwnd.0, pid },
            origin: point,
            latest: Mutex::new((point, false)),
        });
        if send_work(Work::Drag(gesture.clone())) {
            *capture = Some(gesture);
            true
        } else {
            false
        }
    })
}

fn send_work(work: Work) -> bool {
    WORKER.with(|sender| {
        sender
            .borrow()
            .as_ref()
            .is_some_and(|send| send.try_send(work).is_ok())
    })
}

fn unmodified() -> bool {
    let held = |key: u16| unsafe { GetAsyncKeyState(key.into()) } < 0;
    ![VK_MENU, VK_CONTROL, VK_SHIFT, VK_LWIN, VK_RWIN]
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

struct DragWindow {
    hwnd: HWND,
    pid: u32,
    raw: MonitorRect,
    frame: MonitorRect,
}

impl DragWindow {
    fn key(&self) -> WindowKey {
        WindowKey {
            hwnd: self.hwnd.0,
            pid: self.pid,
        }
    }

    fn new(hwnd: HWND) -> Option<Self> {
        let pid = window_pid(hwnd);
        let mut raw = RECT::default();
        if pid == 0 || unsafe { GetWindowRect(hwnd, &mut raw) }.is_err() {
            return None;
        }
        Some(Self {
            hwnd,
            pid,
            raw: raw.into(),
            frame: visibility::visible_frame(hwnd).unwrap_or_else(|| raw.into()),
        })
    }

    fn apply_placement(&self, placement: &Placement) -> bool {
        let rect = placement.rect(&self.raw);
        let (width, height) = rect.size();
        let mut flags = SWP_ASYNCWINDOWPOS | SWP_NOZORDER | SWP_NOACTIVATE;
        if !placement.resizes() {
            flags |= SWP_NOSIZE;
        }
        normal_window(self.hwnd)
            && window_pid(self.hwnd) == self.pid
            && unsafe {
                SetWindowPos(
                    self.hwnd,
                    HWND::default(),
                    rect.left,
                    rect.top,
                    width.max(1),
                    height.max(1),
                    flags,
                )
            }
            .is_ok()
    }
}

fn monitor_area(monitor: HMONITOR) -> Option<MonitorArea> {
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    unsafe { GetMonitorInfoW(monitor, &mut info) }
        .as_bool()
        .then(|| MonitorArea {
            bounds: info.rcMonitor.into(),
            work: info.rcWork.into(),
        })
}

fn run_gesture(gesture: &Gesture, groups: &mut Vec<Vec<WindowKey>>) {
    let Some(options) = OPTIONS.lock().unwrap().clone() else {
        return;
    };
    if !gesture.target.is_current() {
        return;
    }
    let target = HWND(gesture.target.hwnd);
    let source_monitor = unsafe { MonitorFromWindow(target, MONITOR_DEFAULTTONULL) };
    let previous = groups
        .iter()
        .find(|group| group.contains(&gesture.target))
        .map(Vec::as_slice)
        .unwrap_or_default();
    let mut windows = window::get_target_windows()
        .into_iter()
        .filter(|w| !w.is_minimized && !w.is_maximized && !w.is_fullscreen)
        .filter(|w| !pin::is_pinned(w.hwnd) && !is_excluded(w, &options.rules))
        .filter(|w| {
            w.hwnd == gesture.target.hwnd
                || options.follow
                    && (previous.iter().any(|key| key.hwnd == w.hwnd)
                        || source_monitor.0 != 0
                            && unsafe { MonitorFromWindow(HWND(w.hwnd), MONITOR_DEFAULTTONULL) }
                                == source_monitor)
        })
        .filter_map(|w| DragWindow::new(HWND(w.hwnd)))
        .collect::<Vec<_>>();
    let Some(index) = windows.iter().position(|w| w.key() == gesture.target) else {
        return;
    };
    let lead = windows.remove(index);
    let mut members = if options.follow {
        let visibility = visibility::Visibility::capture();
        let lead_is_visible = visibility.has_visible_half(lead.hwnd);
        windows.retain(|candidate| {
            previous.contains(&candidate.key())
                || lead_is_visible && visibility.has_visible_half(candidate.hwnd)
        });
        collect_group(lead, windows, previous)
    } else {
        vec![lead]
    };
    let mut bounds = members[0].frame.clone();
    for member in &members[1..] {
        bounds.left = bounds.left.min(member.frame.left);
        bounds.top = bounds.top.min(member.frame.top);
        bounds.right = bounds.right.max(member.frame.right);
        bounds.bottom = bounds.bottom.max(member.frame.bottom);
    }
    let layout = DragLayout {
        bounds,
        origin: (gesture.origin.x, gesture.origin.y),
        source: monitor_area(source_monitor),
        snap: options.snap,
        resize: options.resize,
    };
    let mut moved = false;
    track_gesture(gesture, &ENABLED, |point, finished| {
        let destination = monitor_area(unsafe { MonitorFromPoint(point, MONITOR_DEFAULTTONULL) });
        let placement = layout.placement((point.x, point.y), destination.as_ref(), finished);
        if !members[0].apply_placement(&placement) {
            return false;
        }
        members
            .retain(|window| window.key() == gesture.target || window.apply_placement(&placement));
        moved = true;
        true
    });
    if options.follow && moved {
        remember_group(groups, members.iter().map(DragWindow::key).collect());
    }
}

fn remember_group(groups: &mut Vec<Vec<WindowKey>>, members: Vec<WindowKey>) {
    groups.retain_mut(|group| {
        group.retain(|key| !members.contains(key));
        group.len() > 1
    });
    if members.len() > 1 {
        groups.push(members);
    }
}

fn is_title_bar(hwnd: HWND, point: POINT, timeout_ms: u32) -> bool {
    // 負座標を含むスクリーン座標を WM_NCHITTEST の符号付き16ビット2成分へ格納する。
    let (Ok(x), Ok(y)) = (i16::try_from(point.x), i16::try_from(point.y)) else {
        return false;
    };
    let packed = (u32::from(y as u16) << 16) | u32::from(x as u16);
    let mut hit = 0usize;
    let result = unsafe {
        SendMessageTimeoutW(
            hwnd,
            WM_NCHITTEST,
            WPARAM(0),
            LPARAM(packed as isize),
            SMTO_ABORTIFHUNG | SMTO_BLOCK | SMTO_ERRORONEXIT,
            timeout_ms,
            Some(&mut hit),
        )
    };
    result.0 != 0 && hit == HTCAPTION as usize
}

fn raise_group(groups: &[Vec<WindowKey>], target: WindowKey) {
    let Some(group) = groups.iter().find(|group| group.contains(&target)) else {
        return;
    };
    // 通常のクリックで選択されたウィンドウのフォーカスを保ち、同じグループだけを上げる。
    for key in group
        .iter()
        .copied()
        .filter(|key| *key != target)
        .chain(std::iter::once(target))
    {
        let member = HWND(key.hwnd);
        if key.is_current()
            && unsafe { IsWindowVisible(member).as_bool() && !IsIconic(member).as_bool() }
        {
            let _ = unsafe {
                SetWindowPos(
                    member,
                    HWND_TOP,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE
                        | SWP_NOSIZE
                        | SWP_NOACTIVATE
                        | SWP_NOOWNERZORDER
                        | SWP_ASYNCWINDOWPOS,
                )
            };
        }
    }
}

fn track_gesture(
    gesture: &Gesture,
    enabled: &AtomicU8,
    mut move_to: impl FnMut(POINT, bool) -> bool,
) {
    let mut last = (0, 0);
    let mut dragging = false;
    // フックが押下を消費するため、ボタンの状態はフックで受け取ったイベントで追跡する。
    while enabled.load(Ordering::Acquire) & MIDDLE_DRAG != 0 {
        let (point, finished) = *gesture.latest.lock().unwrap();
        let offset = (point.x - gesture.origin.x, point.y - gesture.origin.y);
        if (offset != last || finished && dragging)
            && (dragging || offset.0.abs().max(offset.1.abs()) >= DRAG_DISTANCE)
        {
            if !move_to(point, finished) {
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

fn collect_group(
    lead: DragWindow,
    mut candidates: Vec<DragWindow>,
    previous: &[WindowKey],
) -> Vec<DragWindow> {
    let mut members = vec![lead];
    let mut index = 0;
    while index < candidates.len() && members.len() < MAX_GROUP_WINDOWS {
        if previous.contains(&candidates[index].key()) {
            members.push(candidates.remove(index));
        } else {
            index += 1;
        }
    }
    while members.len() < MAX_GROUP_WINDOWS {
        let next = members
            .iter()
            .flat_map(|member| {
                candidates
                    .iter()
                    .enumerate()
                    .filter_map(move |(index, candidate)| {
                        adjacent_distance(&member.frame, &candidate.frame)
                            .or_else(|| adjacent_distance(&member.raw, &candidate.raw))
                            .map(|distance| (index, distance))
                    })
            })
            .min_by_key(|(_, distance)| *distance);
        let Some((index, _)) = next else { break };
        members.push(candidates.remove(index));
    }
    members
}

fn adjacent_distance(left: &MonitorRect, right: &MonitorRect) -> Option<i32> {
    let overlaps = |a_start: i32, a_end: i32, b_start: i32, b_end: i32| {
        let overlap = a_end.min(b_end) - a_start.max(b_start);
        overlap > 0 && overlap * 2 >= (a_end - a_start).min(b_end - b_start)
    };
    let horizontal = overlaps(left.top, left.bottom, right.top, right.bottom).then(|| {
        (left.right - right.left)
            .abs()
            .min((left.left - right.right).abs())
    });
    let vertical = overlaps(left.left, left.right, right.left, right.right).then(|| {
        (left.bottom - right.top)
            .abs()
            .min((left.top - right.bottom).abs())
    });
    horizontal
        .into_iter()
        .chain(vertical)
        .min()
        .filter(|distance| *distance <= SNAP_DISTANCE)
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
        assert!(!config.middle_drag_enabled && !config.alt_drag_follow_neighbors);
        assert!(config.drag_snap_to_work_area && config.drag_resize_across_monitors);
        let hook = install_hook().expect("Windowsのマウスフックを登録");
        unsafe { UnhookWindowsHookEx(hook) }.expect("試験フックを解除");
    }

    #[test]
    fn only_the_native_caption_is_a_drag_handle() {
        use windows::core::w;
        use windows::Win32::Graphics::Gdi::ClientToScreen;
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, UnregisterClassW,
            HMENU, WINDOW_EX_STYLE, WNDCLASSW, WS_OVERLAPPEDWINDOW,
        };

        unsafe extern "system" fn window_proc(
            hwnd: HWND,
            message: u32,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> LRESULT {
            DefWindowProcW(hwnd, message, wparam, lparam)
        }

        unsafe {
            let name = w!("ChuwitchWindowCaptionHitTest");
            let instance = HINSTANCE(GetModuleHandleW(None).unwrap().0);
            assert_ne!(
                RegisterClassW(&WNDCLASSW {
                    lpfnWndProc: Some(window_proc),
                    hInstance: instance,
                    lpszClassName: name,
                    ..Default::default()
                }),
                0
            );
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                name,
                name,
                WS_OVERLAPPEDWINDOW,
                -500,
                100,
                400,
                260,
                HWND::default(),
                HMENU::default(),
                instance,
                None,
            );
            assert_ne!(hwnd.0, 0);
            let mut content = POINT { x: 160, y: 0 };
            assert!(ClientToScreen(hwnd, &mut content).as_bool());
            let caption = POINT {
                x: content.x,
                y: content.y - 6,
            };
            let inside = POINT {
                x: content.x,
                y: content.y + 40,
            };
            let hits = (
                is_title_bar(hwnd, caption, TITLE_HIT_TIMEOUT_MS),
                is_title_bar(hwnd, inside, TITLE_HIT_TIMEOUT_MS),
            );
            DestroyWindow(hwnd).unwrap();
            UnregisterClassW(name, instance).unwrap();
            assert_eq!(hits, (true, false));
        }
    }

    #[test]
    fn captured_drag_processes_multiple_moves_and_the_release_position() {
        assert_drag_moves(POINT { x: 25, y: 20 });
        assert_drag_moves(POINT { x: 20, y: 15 });
    }

    fn assert_drag_moves(release_point: POINT) {
        let gesture = Arc::new(Gesture {
            target: WindowKey { hwnd: 0, pid: 0 },
            origin: POINT { x: 0, y: 0 },
            latest: Mutex::new((POINT { x: 0, y: 0 }, false)),
        });
        CAPTURE.with(|slot| slot.replace(Some(gesture.clone())));
        assert!(!record_mouse(WM_MOUSEMOVE, POINT { x: 10, y: 5 }));

        let enabled = AtomicU8::new(MIDDLE_DRAG);
        let mut moves = Vec::new();
        track_gesture(&gesture, &enabled, |point, finished| {
            moves.push((point.x, point.y, finished));
            match moves.len() {
                1 => {
                    assert!(!record_mouse(WM_LBUTTONDOWN, POINT { x: 99, y: 99 }));
                    assert!(!record_mouse(WM_MOUSEMOVE, POINT { x: 20, y: 15 }));
                }
                2 => assert!(record_mouse(WM_MBUTTONUP, release_point)),
                _ => {}
            }
            true
        });

        assert_eq!(
            moves,
            [
                (10, 5, false),
                (20, 15, false),
                (release_point.x, release_point.y, true)
            ]
        );
        assert!(!record_mouse(WM_MOUSEMOVE, POINT { x: 30, y: 30 }));
    }

    #[test]
    fn only_nearby_edges_with_sufficient_overlap_group() {
        let lead = rect(0, 0, 100, 100);
        assert_eq!(adjacent_distance(&lead, &rect(112, 20, 190, 120)), Some(12));
        assert_eq!(adjacent_distance(&lead, &rect(-80, 0, -5, 100)), Some(5));
        assert_eq!(adjacent_distance(&lead, &rect(25, 105, 150, 195)), Some(5));
        assert_eq!(adjacent_distance(&lead, &rect(113, 20, 190, 120)), None);
        assert_eq!(adjacent_distance(&lead, &rect(100, 90, 175, 175)), None);
        assert_eq!(adjacent_distance(&lead, &lead), None);
    }

    fn fixture(index: isize, raw: MonitorRect, frame: MonitorRect) -> DragWindow {
        DragWindow {
            hwnd: HWND(index),
            pid: 1,
            raw,
            frame,
        }
    }

    #[test]
    fn collects_eight_before_moving_and_preserves_original_rectangles() {
        let lead = fixture(0, rect(0, 0, 100, 100), rect(5, 0, 95, 95));
        let candidates = (1..=8)
            .rev()
            .map(|index| {
                let x = index * 104;
                fixture(
                    index as isize,
                    rect(x, 0, x + 100, 100),
                    rect(x + 5, 0, x + 95, 95),
                )
            })
            .collect();
        let members = collect_group(lead, candidates, &[]);
        assert_eq!(members.len(), 8);
        for (index, member) in members.iter().enumerate() {
            assert_eq!(member.hwnd.0, index as isize);
            assert_eq!(member.raw.left, index as i32 * 104);
        }
    }

    #[test]
    fn existing_members_keep_their_places_and_can_be_reassigned() {
        let lead = fixture(1, rect(0, 0, 100, 100), rect(0, 0, 100, 100));
        let member = fixture(2, rect(200, 0, 300, 100), rect(200, 0, 300, 100));
        let previous = vec![lead.key(), member.key()];
        let members = collect_group(lead, vec![member], &previous);
        assert_eq!(members.len(), 2);
        assert_eq!(members[1].raw.left, 200);
        let mut groups = vec![previous.clone()];
        let other = WindowKey { hwnd: 3, pid: 1 };
        remember_group(&mut groups, vec![previous[1], other]);
        assert_eq!(groups, [vec![previous[1], other]]);
    }
}
