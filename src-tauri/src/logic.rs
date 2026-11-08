use crate::{
    history::{HistoryState, MoveAction},
    monitor::{get_all_monitors, MonitorInfo},
    window::{get_target_windows, move_windows, WindowInfo},
};
use std::collections::HashMap;
use tauri::Manager;

// --- Pure Logic for Testing ---
pub fn group_windows_by_monitor(
    windows: &[WindowInfo],
    monitors: &[MonitorInfo]
) -> Vec<Vec<WindowInfo>> {
    let mut windows_by_monitor: Vec<Vec<WindowInfo>> = vec![Vec::new(); monitors.len()];
    
    for win in windows.iter() {
        let mut matched_idx = 0;
        let mut max_area = 0;
        
        for (i, m) in monitors.iter().enumerate() {
            let intersect_left = win.rect.left.max(m.monitor_area.left);
            let intersect_right = win.rect.right.min(m.monitor_area.right);
            let intersect_top = win.rect.top.max(m.monitor_area.top);
            let intersect_bottom = win.rect.bottom.min(m.monitor_area.bottom);
            
            if intersect_right > intersect_left && intersect_bottom > intersect_top {
                let area = (intersect_right - intersect_left) * (intersect_bottom - intersect_top);
                if area > max_area {
                    max_area = area;
                    matched_idx = i;
                }
            }
        }
        
        windows_by_monitor[matched_idx].push(win.clone());
    }
    
    windows_by_monitor
}

fn rotate_routes(
    monitors: &[MonitorInfo],
    groups: Option<&HashMap<String, u8>>,
    clockwise: bool,
) -> Vec<(usize, usize)> {
    let mut cycles = HashMap::<u8, Vec<usize>>::new();
    for (index, monitor) in monitors.iter().enumerate() {
        let group = groups.and_then(|groups| groups.get(&monitor.name)).copied().unwrap_or(0);
        cycles.entry(group).or_default().push(index);
    }
    cycles.values().filter(|cycle| cycle.len() > 1 || groups.is_none())
        .flat_map(|cycle| cycle.iter().enumerate().map(move |(index, source)| {
            let offset = if clockwise { 1 } else { cycle.len() - 1 };
            (*source, cycle[(index + offset) % cycle.len()])
        }))
        .collect()
}

fn swap_pair(monitors: &[MonitorInfo], primary_name: Option<&str>, display: u32) -> Option<(usize, usize)> {
    let primary = monitors.iter().position(|m| primary_name.map_or(m.is_primary, |name| m.name == name))?;
    let target = monitors.iter().position(|m| m.display_number == display)?;
    (primary != target).then_some((primary, target))
}

fn escape_destination(monitors: &[MonitorInfo], active: usize) -> Option<usize> {
    let primary = monitors[active].is_primary;
    let destination = monitors.iter().position(|monitor| monitor.is_primary != primary).unwrap_or(0);
    (active != destination).then_some(destination)
}

fn gather_routes(monitor_count: usize, active: usize) -> impl Iterator<Item = (usize, usize)> {
    (0..monitor_count).filter(move |source| *source != active).map(move |source| (source, active))
}

fn move_between(
    windows: &[Vec<WindowInfo>],
    monitors: &[MonitorInfo],
    routes: impl IntoIterator<Item = (usize, usize)>,
) {
    for (source, destination) in routes {
        if !windows[source].is_empty() {
            move_windows(&windows[source], &monitors[source], &monitors[destination]);
        }
    }
}

// --- App Logic ---

pub fn handle_rotate(app: &tauri::AppHandle, clockwise: bool) {
    tracing::info!("Exec [handle_rotate]: clockwise={}", clockwise);
    let mut monitors = get_all_monitors();
    if monitors.is_empty() {
        return;
    }
    let config = crate::config::load_config();
    let parsed_rules = crate::rule::parse_rules(&config.exclusion_rules);
    
    // config.monitor_order に基づきソート。未指定のものは末尾に（X座標でフォールバックソート）
    monitors.sort_by(|a, b| {
        let pos_a = config.monitor_order.iter().position(|name| name == &a.name);
        let pos_b = config.monitor_order.iter().position(|name| name == &b.name);
        
        match (pos_a, pos_b) {
            (Some(ia), Some(ib)) => ia.cmp(&ib),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.monitor_area.left.cmp(&b.monitor_area.left),
        }
    });
    
    let windows: Vec<WindowInfo> = get_target_windows()
        .into_iter()
        .filter(|w| {
            if crate::pin::is_pinned(w.hwnd) {
                tracing::info!("Excluded by Pin: {} ({})", w.title, w.hwnd);
                false
            } else { true }
        })
        .filter(|w| {
            if crate::rule::is_excluded(w, &parsed_rules) {
                tracing::info!("Excluded by Rule: {} ({})", w.title, w.hwnd);
                false
            } else { true }
        })
        .collect();
    if windows.is_empty() {
        return;
    }

    let history_state = app.state::<HistoryState>();
    let mut actions = Vec::new();

    for win in windows.iter() {
        actions.push(MoveAction {
            hwnd: win.hwnd,
            old_rect: win.rect.clone(),
            new_rect: win.rect.clone(),
        });
    }
    history_state.push(actions);

    let windows_by_monitor = group_windows_by_monitor(&windows, &monitors);
    
    let groups = config.swap_within_groups.then_some(&config.monitor_groups);
    move_between(&windows_by_monitor, &monitors, rotate_routes(&monitors, groups, clockwise));
}

pub fn handle_undo(app: &tauri::AppHandle) {
    tracing::info!("Exec [handle_undo]");
    let history_state = app.state::<HistoryState>();
    restore_last_move(&history_state);
}

fn restore_last_move(history_state: &HistoryState) {
    if let Some(actions) = history_state.pop() {
        use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, HWND_TOP};
        use windows::Win32::Foundation::HWND;

        // Undo is just shifting back to old_rect exact coordinates
        // Preserve Z-order ideally, but for now absolute position is restored
        for action in actions.iter().rev() {
            let hwnd = HWND(action.hwnd as isize);
            let w = action.old_rect.right - action.old_rect.left;
            let h = action.old_rect.bottom - action.old_rect.top;
            unsafe {
                let _ = SetWindowPos(
                    hwnd,
                    HWND_TOP,
                    action.old_rect.left,
                    action.old_rect.top,
                    w,
                    h,
                    SWP_NOACTIVATE,
                );
            }
        }
    }
}

pub fn handle_swap_target(app: &tauri::AppHandle, target_num: u32) {
    tracing::info!("Exec [handle_swap_target]: target_num={}", target_num);
    let config = crate::config::load_config();
    let mut monitors = get_all_monitors();
    if monitors.len() < 2 {
        return;
    }
    monitors.sort_by_key(|m| m.monitor_area.left);

    let Some((primary_idx, target_idx)) = swap_pair(&monitors, config.primary_monitor_id.as_deref(), target_num) else {
        return;
    };
    let parsed_rules = crate::rule::parse_rules(&config.exclusion_rules);

    let windows: Vec<WindowInfo> = get_target_windows()
        .into_iter()
        .filter(|w| {
            if crate::pin::is_pinned(w.hwnd) {
                tracing::info!("Excluded by Pin: {} ({})", w.title, w.hwnd);
                false
            } else { true }
        })
        .filter(|w| {
            if crate::rule::is_excluded(w, &parsed_rules) {
                tracing::info!("Excluded by Rule: {} ({})", w.title, w.hwnd);
                false
            } else { true }
        })
        .collect();
    if windows.is_empty() {
        return;
    }

    let history_state = app.state::<HistoryState>();
    let mut actions = Vec::new();
    for win in windows.iter() {
        actions.push(MoveAction {
            hwnd: win.hwnd,
            old_rect: win.rect.clone(),
            new_rect: win.rect.clone(),
        });
    }
    history_state.push(actions);

    let windows_by_monitor = group_windows_by_monitor(&windows, &monitors);

    move_between(&windows_by_monitor, &monitors, [(primary_idx, target_idx), (target_idx, primary_idx)]);
}



pub fn handle_escape(app: &tauri::AppHandle) {
    tracing::info!("Exec [handle_escape]");
    let monitors = get_all_monitors();
    if monitors.len() < 2 { return; }
    
    let config = crate::config::load_config();
    let active_idx = match crate::monitor::get_active_monitor_index(&monitors) {
        Some(idx) => idx,
        None => return,
    };
    
    // 退避先モニターの決定 (プライマリ優先、自身がプライマリなら他のモニター)
    let Some(target_idx) = escape_destination(&monitors, active_idx) else { return; };    let parsed_rules = crate::rule::parse_rules(&config.exclusion_rules);
    
    let windows: Vec<WindowInfo> = get_target_windows()
        .into_iter()
        .filter(|w| !crate::pin::is_pinned(w.hwnd))
        .filter(|w| !crate::rule::is_excluded(w, &parsed_rules)) // v0.2: ルール除外
        .collect();
    if windows.is_empty() { return; }
        
    let history_state = app.state::<HistoryState>();
    let mut actions = Vec::new();
    for win in windows.iter() {
        actions.push(MoveAction {
            hwnd: win.hwnd,
            old_rect: win.rect.clone(),
            new_rect: win.rect.clone(),
        });
    }
    history_state.push(actions);
    
    let windows_by_monitor = group_windows_by_monitor(&windows, &monitors);
    move_between(&windows_by_monitor, &monitors, [(active_idx, target_idx)]);
}

pub fn handle_gather(app: &tauri::AppHandle) {
    tracing::info!("Exec [handle_gather]");
    let monitors = get_all_monitors();
    if monitors.len() < 2 { return; }
    
    let config = crate::config::load_config();
    let active_idx = match crate::monitor::get_active_monitor_index(&monitors) {
        Some(idx) => idx,
        None => return,
    };
    
    let parsed_rules = crate::rule::parse_rules(&config.exclusion_rules);
    
    let windows: Vec<WindowInfo> = get_target_windows()
        .into_iter()
        .filter(|w| !crate::pin::is_pinned(w.hwnd))
        .filter(|w| !crate::rule::is_excluded(w, &parsed_rules)) // v0.2: ルール除外
        .collect();
    if windows.is_empty() { return; }
        
    let history_state = app.state::<HistoryState>();
    let mut actions = Vec::new();
    for win in windows.iter() {
        actions.push(MoveAction {
            hwnd: win.hwnd,
            old_rect: win.rect.clone(),
            new_rect: win.rect.clone(),
        });
    }
    history_state.push(actions);
    
    let windows_by_monitor = group_windows_by_monitor(&windows, &monitors);
    
    move_between(&windows_by_monitor, &monitors, gather_routes(monitors.len(), active_idx));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{dual_setup, monitor_1920, test_window, WindowBuilder};

    #[test]
    fn selects_routes_for_rotation_swap_escape_and_gather() {
        let mut monitors = dual_setup();
        let mut third = monitor_1920("M3", -1920);
        third.display_number = 3;
        monitors.push(third);

        assert_eq!(rotate_routes(&monitors, None, true), [(0, 1), (1, 2), (2, 0)]);
        assert_eq!(rotate_routes(&monitors, None, false), [(0, 2), (1, 0), (2, 1)]);
        let groups = HashMap::from([("M1".into(), 1), ("M3".into(), 1)]);
        assert_eq!(rotate_routes(&monitors, Some(&groups), true), [(0, 2), (2, 0)]);
        assert_eq!(swap_pair(&monitors, None, 3), Some((0, 2)));
        assert_eq!(swap_pair(&monitors, Some("M3"), 2), Some((2, 1)));
        assert_eq!(swap_pair(&monitors, Some("M2"), 2), None);
        assert_eq!(swap_pair(&monitors, Some("unknown"), 2), None);
        assert_eq!(escape_destination(&monitors, 0), Some(1));
        assert_eq!(escape_destination(&monitors, 2), Some(0));
        assert_eq!(gather_routes(monitors.len(), 1).collect::<Vec<_>>(), [(0, 1), (2, 1)]);
    }

    #[test]
    fn move_actions_apply_coordinates_and_undo_restores_test_windows() {
        use crate::monitor::MonitorRect;
        use windows::Win32::Foundation::RECT;
        use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, WS_OVERLAPPEDWINDOW};

        let mut monitors = [monitor_1920("M1", -16000), monitor_1920("M2", -14000)].map(|mut monitor| {
            for area in [&mut monitor.monitor_area, &mut monitor.work_area] {
                area.top -= 16000;
                area.bottom -= 16000;
            }
            monitor
        });
        monitors[0].is_primary = true;
        monitors[0].display_number = 1;
        let first = test_window(WS_OVERLAPPEDWINDOW, -15900, -15800);
        let second = test_window(WS_OVERLAPPEDWINDOW, -13800, -15700);
        let read = |hwnd| -> MonitorRect {
            let mut rect = RECT::default();
            unsafe { GetWindowRect(hwnd, &mut rect) }.unwrap();
            rect.into()
        };
        let originals = [read(first.0), read(second.0)];
        let windows = [first.0, second.0].into_iter().zip(&originals).map(|(hwnd, rect)| {
            WindowBuilder::new(hwnd.0).rect(rect.left, rect.top, rect.right, rect.bottom).build()
        }).collect::<Vec<_>>();
        let by_monitor = group_windows_by_monitor(&windows, &monitors);
        let history = HistoryState::new();
        let (primary, secondary) = swap_pair(&monitors, None, 2).unwrap();
        for (routes, expected_x) in [
            (rotate_routes(&monitors, None, true), [-13900, -15800]),
            (vec![(primary, secondary), (secondary, primary)], [-13900, -15800]),
            (vec![(0, escape_destination(&monitors, 0).unwrap())], [-13900, -13800]),
            (gather_routes(2, 0).collect(), [-15900, -15800]),
        ] {
            history.push(windows.iter().map(|window| MoveAction {
                hwnd: window.hwnd,
                old_rect: window.rect.clone(),
                new_rect: window.rect.clone(),
            }).collect());
            move_between(&by_monitor, &monitors, routes);
            let after = [read(first.0), read(second.0)];
            assert_eq!(after.each_ref().map(|rect| rect.left), expected_x);
            assert_eq!(after.each_ref().map(|rect| (rect.top, rect.size())),
                       originals.each_ref().map(|rect| (rect.top, rect.size())));
            restore_last_move(&history);
            assert_eq!([read(first.0), read(second.0)], originals);
            assert!(history.pop().is_none());
        }
    }

    #[test]
    fn test_group_windows_by_monitor() {
        let monitors = dual_setup();
        
        let windows = vec![
            WindowBuilder::new(1).rect(100, 100, 500, 500).build(),
            WindowBuilder::new(2).rect(2000, 100, 2500, 500).build(),
            WindowBuilder::new(3).rect(-100, -100, 50, 50).build(),
        ];

        let grouped = group_windows_by_monitor(&windows, &monitors);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].len(), 2); // w1, w3
        assert_eq!(grouped[1].len(), 1); // w2
    }
}
