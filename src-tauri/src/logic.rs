use crate::{
    history::{HistoryState, MoveAction},
    monitor::{get_all_monitors, MonitorInfo},
    window::{get_target_windows, move_windows, WindowInfo},
};
use std::collections::HashMap;
use tauri::{Manager, Emitter};

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

// 削除

// --- App Logic ---

pub fn handle_rotate(app: &tauri::AppHandle, clockwise: bool) {
    let mut monitors = get_all_monitors();
    if monitors.is_empty() {
        return;
    }
    let config = crate::config::load_config();
    
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
    
    let windows = get_target_windows();
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
    
    if config.swap_within_groups {
        // グループごとに独立してシフトする
        let mut groups: HashMap<u8, Vec<usize>> = HashMap::new();
        for (i, m) in monitors.iter().enumerate() {
            let group_id = config.monitor_groups.get(&m.name).copied().unwrap_or(0);
            groups.entry(group_id).or_default().push(i);
        }

        for (_group_id, monitor_indices) in groups {
            if monitor_indices.len() < 2 {
                continue;
            }

            for (idx, &curr_monitor_idx) in monitor_indices.iter().enumerate() {
                let wins = &windows_by_monitor[curr_monitor_idx];
                if wins.is_empty() {
                    continue;
                }

                let next_idx_in_group = if clockwise {
                    (idx + 1) % monitor_indices.len()
                } else {
                    (idx + monitor_indices.len() - 1) % monitor_indices.len()
                };
                let next_monitor_idx = monitor_indices[next_idx_in_group];

                move_windows(wins, &monitors[curr_monitor_idx], &monitors[next_monitor_idx]);
            }
        }
    } else {
        // 全体を1つのリストとしてシフトする
        for (i, wins) in windows_by_monitor.iter().enumerate() {
            if wins.is_empty() {
                continue;
            }
            
            let next_i = if clockwise {
                (i + 1) % monitors.len()
            } else {
                (i + monitors.len() - 1) % monitors.len()
            };
            
            move_windows(wins, &monitors[i], &monitors[next_i]);
        }
    }
    
    let _ = app.emit("osd-notify", if clockwise { "順方向に入れ替えました" } else { "逆方向に入れ替えました" });
}

pub fn handle_undo(app: &tauri::AppHandle) {
    let history_state = app.state::<HistoryState>();
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
        let _ = app.emit("osd-notify", "元に戻しました");
    } else {
        let _ = app.emit("osd-notify", "履歴がありません");
    }
}

// 削除
pub fn handle_swap_target(app: &tauri::AppHandle, target_num: u32) {
    let config = crate::config::load_config();
    let mut monitors = get_all_monitors();
    if monitors.len() < 2 {
        return;
    }
    monitors.sort_by_key(|m| m.monitor_area.left);

    // Identify primary monitor
    let primary_idx = monitors.iter().position(|m| {
        if let Some(ref pid) = config.primary_monitor_id {
            m.name == *pid
        } else {
            m.is_primary
        }
    });

    let primary_idx = match primary_idx {
        Some(idx) => idx,
        None => return,
    };

    // Identify target monitor
    let target_idx = monitors.iter().position(|m| m.display_number == target_num);
    
    let target_idx = match target_idx {
        Some(idx) => idx,
        None => return, // target display not found
    };

    if primary_idx == target_idx {
        return; // same monitor
    }

    let windows = get_target_windows();
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

    let primary_wins = &windows_by_monitor[primary_idx];
    let target_wins = &windows_by_monitor[target_idx];

    let mut swapped_any = false;
    if !primary_wins.is_empty() {
        move_windows(primary_wins, &monitors[primary_idx], &monitors[target_idx]);
        swapped_any = true;
    }
    if !target_wins.is_empty() {
        move_windows(target_wins, &monitors[target_idx], &monitors[primary_idx]);
        swapped_any = true;
    }

    if swapped_any {
        let msg = format!("メイン ⇄ 画面 {}", target_num);
        let _ = app.emit("osd-notify", msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::RECT;

    fn create_mock_monitor(name: &str, left: i32, top: i32, right: i32, bottom: i32) -> MonitorInfo {
        MonitorInfo {
            hmonitor: 0,
            name: name.to_string(),
            display_number: 1,
            manufacturer: "".to_string(),
            serial_number: "".to_string(),
            work_area: RECT { left, top, right, bottom }.into(),
            monitor_area: RECT { left, top, right, bottom }.into(),
            is_primary: false,
        }
    }

    fn create_mock_window(hwnd_id: isize, left: i32, top: i32, right: i32, bottom: i32) -> WindowInfo {
        WindowInfo {
            hwnd: hwnd_id,
            title: "Mock Window".to_string(),
            rect: RECT { left, top, right, bottom }.into(),
            dpi: 96,
            is_maximized: false,
            is_minimized: false,
            is_fullscreen: false,
        }
    }

    #[test]
    fn test_group_windows_by_monitor() {
        let monitors = vec![
            create_mock_monitor("M1", 0, 0, 1920, 1080),
            create_mock_monitor("M2", 1920, 0, 3840, 1080),
        ];
        
        let windows = vec![
            create_mock_window(1, 100, 100, 500, 500), // M1
            create_mock_window(2, 2000, 100, 2500, 500), // M2
            create_mock_window(3, -100, -100, 50, 50), // M1 (intersect 50x50 with M1)
        ];

        let grouped = group_windows_by_monitor(&windows, &monitors);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].len(), 2); // w1, w3
        assert_eq!(grouped[1].len(), 1); // w2
    }

    // 削除
}
