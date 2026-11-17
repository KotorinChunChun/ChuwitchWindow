use crate::{
    config::AppConfig,
    history::HistoryState,
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

/// config.monitor_order に基づきソートする。未指定のものは末尾に（X座標でフォールバックソート）
pub fn sort_by_monitor_order(monitors: &mut [MonitorInfo], monitor_order: &[String]) {
    monitors.sort_by(|a, b| {
        let pos_a = monitor_order.iter().position(|name| name == &a.name);
        let pos_b = monitor_order.iter().position(|name| name == &b.name);

        match (pos_a, pos_b) {
            (Some(ia), Some(ib)) => ia.cmp(&ib),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.monitor_area.left.cmp(&b.monitor_area.left),
        }
    });
}

/// プライマリモニターのインデックスを返す。
/// 設定画面でユーザーが指定したモニターを優先し、未指定なら OS のプライマリモニターとする。
pub fn resolve_primary_index(monitors: &[MonitorInfo], primary_monitor_id: Option<&str>) -> Option<usize> {
    match primary_monitor_id {
        Some(pid) => monitors.iter().position(|m| m.name == pid),
        None => monitors.iter().position(|m| m.is_primary),
    }
}

/// 一斉退避の移動先を返す。
/// アクティブモニターがプライマリでなければプライマリへ、プライマリ自身なら他のモニターへ退避する。
pub fn resolve_escape_target(monitors: &[MonitorInfo], active_idx: usize, primary_idx: Option<usize>) -> Option<usize> {
    match primary_idx {
        Some(p) if p != active_idx => Some(p),
        _ => (0..monitors.len()).find(|&i| i != active_idx),
    }
}

/// シフト・入れ替えの移動対象（ピン留め・除外ルールに該当しないウィンドウ）を Z オーダー順に取得する
fn collect_movable_windows(config: &AppConfig) -> Vec<WindowInfo> {
    let parsed_rules = crate::rule::parse_rules(&config.exclusion_rules);
    get_target_windows(config.ignore_fullscreen, config.exclude_minimized)
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
        .collect()
}

/// 移動対象を取得し、Undo 用に移動前の位置を記録したうえでモニターごとに振り分ける。
/// 移動対象が無ければ `None`（履歴も積まない）。
fn prepare_move(app: &tauri::AppHandle, config: &AppConfig, monitors: &[MonitorInfo]) -> Option<Vec<Vec<WindowInfo>>> {
    let windows = collect_movable_windows(config);
    if windows.is_empty() {
        return None;
    }
    app.state::<HistoryState>().push_windows(&windows);
    Some(group_windows_by_monitor(&windows, monitors))
}

// --- App Logic ---

pub fn handle_rotate(app: &tauri::AppHandle, clockwise: bool) {
    tracing::info!("Exec [handle_rotate]: clockwise={}", clockwise);
    let mut monitors = get_all_monitors();
    if monitors.is_empty() {
        return;
    }
    let config = crate::config::load_config();
    sort_by_monitor_order(&mut monitors, &config.monitor_order);

    let Some(windows_by_monitor) = prepare_move(app, &config, &monitors) else { return };

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
}

pub fn handle_undo(app: &tauri::AppHandle) {
    tracing::info!("Exec [handle_undo]");
    let history_state = app.state::<HistoryState>();
    if let Some(actions) = history_state.pop() {
        // 移動時と逆順に HWND_TOP へ置き直すことで、Z オーダーも操作前の並びに戻す
        for action in actions.iter().rev() {
            crate::window::place_window(action.hwnd, &action.old_rect, action.was_maximized);
        }
    }
}

pub fn handle_swap_target(app: &tauri::AppHandle, target_num: u32) {
    tracing::info!("Exec [handle_swap_target]: target_num={}", target_num);
    let config = crate::config::load_config();
    let monitors = get_all_monitors();
    if monitors.len() < 2 {
        return;
    }

    let Some(primary_idx) = resolve_primary_index(&monitors, config.primary_monitor_id.as_deref()) else { return };
    // target display not found
    let Some(target_idx) = monitors.iter().position(|m| m.display_number == target_num) else { return };
    if primary_idx == target_idx {
        return; // same monitor
    }

    let Some(windows_by_monitor) = prepare_move(app, &config, &monitors) else { return };

    let primary_wins = &windows_by_monitor[primary_idx];
    let target_wins = &windows_by_monitor[target_idx];
    if !primary_wins.is_empty() {
        move_windows(primary_wins, &monitors[primary_idx], &monitors[target_idx]);
    }
    if !target_wins.is_empty() {
        move_windows(target_wins, &monitors[target_idx], &monitors[primary_idx]);
    }
}

pub fn handle_escape(app: &tauri::AppHandle) {
    tracing::info!("Exec [handle_escape]");
    let monitors = get_all_monitors();
    if monitors.len() < 2 { return; }

    let config = crate::config::load_config();
    let Some(active_idx) = crate::monitor::get_active_monitor_index(&monitors) else { return };
    let primary_idx = resolve_primary_index(&monitors, config.primary_monitor_id.as_deref());
    let Some(target_idx) = resolve_escape_target(&monitors, active_idx, primary_idx) else { return };

    let Some(windows_by_monitor) = prepare_move(app, &config, &monitors) else { return };
    let active_wins = &windows_by_monitor[active_idx];
    if !active_wins.is_empty() {
        move_windows(active_wins, &monitors[active_idx], &monitors[target_idx]);
    }
}

pub fn handle_gather(app: &tauri::AppHandle) {
    tracing::info!("Exec [handle_gather]");
    let monitors = get_all_monitors();
    if monitors.len() < 2 { return; }

    let config = crate::config::load_config();
    let Some(active_idx) = crate::monitor::get_active_monitor_index(&monitors) else { return };

    let Some(windows_by_monitor) = prepare_move(app, &config, &monitors) else { return };
    for (i, wins) in windows_by_monitor.iter().enumerate() {
        if i == active_idx || wins.is_empty() { continue; }
        move_windows(wins, &monitors[i], &monitors[active_idx]);
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
            process_name: "mock.exe".to_string(),
            class_name: "MockClass".to_string(),
            style: 0,
            ex_style: 0,
        }
    }

    /// M1(OS プライマリ) / M2 / M3 を左から並べたモニター構成
    fn three_monitors() -> Vec<MonitorInfo> {
        let mut monitors = vec![
            create_mock_monitor("M1", 0, 0, 1920, 1080),
            create_mock_monitor("M2", 1920, 0, 3840, 1080),
            create_mock_monitor("M3", 3840, 0, 5760, 1080),
        ];
        monitors[0].is_primary = true;
        monitors
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

    #[test]
    fn test_sort_by_monitor_order() {
        let mut monitors = three_monitors();
        // M3 → M1 の順を指定、M2 は未指定なので末尾
        sort_by_monitor_order(&mut monitors, &["M3".to_string(), "M1".to_string()]);
        let names: Vec<&str> = monitors.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["M3", "M1", "M2"]);
    }

    #[test]
    fn test_resolve_primary_index() {
        let monitors = three_monitors();
        // 未指定なら OS のプライマリ
        assert_eq!(resolve_primary_index(&monitors, None), Some(0));
        // ユーザー指定を優先
        assert_eq!(resolve_primary_index(&monitors, Some("M2")), Some(1));
        // 指定したモニターが存在しない（切断済み）
        assert_eq!(resolve_primary_index(&monitors, Some("M9")), None);
    }

    #[test]
    fn test_resolve_escape_target() {
        let monitors = three_monitors();
        // 非プライマリからはプライマリへ退避
        assert_eq!(resolve_escape_target(&monitors, 2, Some(1)), Some(1));
        // プライマリ自身からは他のモニターへ退避
        assert_eq!(resolve_escape_target(&monitors, 1, Some(1)), Some(0));
        assert_eq!(resolve_escape_target(&monitors, 0, Some(0)), Some(1));
        // プライマリが見つからない場合も他のモニターへ退避
        assert_eq!(resolve_escape_target(&monitors, 0, None), Some(1));
    }
}
