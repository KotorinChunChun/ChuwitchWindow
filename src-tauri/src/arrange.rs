use crate::monitor::{MonitorInfo, MonitorRect};
use crate::window::{WindowInfo};
use crate::history::{HistoryState, MoveAction};
use tauri::Manager;
use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, HWND_TOP};
use tracing::info;
use std::sync::{Mutex, OnceLock};

/// ポップアップ表示時に記憶する「整列対象モニターの hmonitor」
/// show_arrange_window() の呼び出し時点（まだ arrange ウィンドウが最前面でない状態）で設定する。
static LAST_TARGET_HMONITOR: OnceLock<Mutex<Option<isize>>> = OnceLock::new();

fn last_target_hmonitor_lock() -> &'static Mutex<Option<isize>> {
    LAST_TARGET_HMONITOR.get_or_init(|| Mutex::new(None))
}

/// ポップアップ表示時に呼び出し、対象モニターの hmonitor を記憶させる
pub fn set_target_hmonitor(hmonitor: isize) {
    *last_target_hmonitor_lock().lock().unwrap() = Some(hmonitor);
    info!("[arrange] 対象モニターを記憶: hmonitor={}", hmonitor);
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum ArrangeType {
    Grid,
    Vertical,
    Horizontal,
    Cascade,
}

/// 自動整列を実行する
pub fn handle_arrange(app: &tauri::AppHandle, arrange_type: ArrangeType) {
    info!("Exec [handle_arrange]: type={:?}", arrange_type);
    
    let monitors = crate::monitor::get_all_monitors();
    if monitors.is_empty() {
        info!("[arrange] モニターが0件のため終了");
        return;
    }

    // ポップアップ表示時に記憶した hmonitor を優先して使う。
    // 表示後は GetForegroundWindow() が arrange ウィンドウ自身を返してしまうため、
    // その段階で呼ぶと対象モニターの判定が狂う。
    let stored_hmonitor = *last_target_hmonitor_lock().lock().unwrap();
    let active_idx = if let Some(hmon) = stored_hmonitor {
        let idx = monitors.iter().position(|m| m.hmonitor == hmon).unwrap_or(0);
        info!("[arrange] 記憶済みhmonitor={} → monitors[{}]={} を対象に決定",
            hmon, idx, monitors[idx].name);
        idx
    } else {
        let idx = get_active_monitor_index(&monitors).unwrap_or(0);
        info!("[arrange] 記憶なし: GetForegroundWindow() → monitors[{}]={} を対象に決定",
            idx, monitors[idx].name);
        idx
    };
    let target_monitor = &monitors[active_idx];
    info!("[arrange] 対象モニター: name={} work_area=({},{})–({},{})",
        target_monitor.name,
        target_monitor.work_area.left, target_monitor.work_area.top,
        target_monitor.work_area.right, target_monitor.work_area.bottom);
    
    let config = crate::config::load_config();
    let parsed_rules = crate::rule::parse_rules(&config.exclusion_rules);

    // 1. 全ウィンドウを取得
    let all_windows = crate::window::get_target_windows(config.ignore_fullscreen, config.exclude_minimized);
    info!("[arrange] get_target_windows() で取得したウィンドウ数: {}", all_windows.len());
    
    // 2. ターゲットモニター上のウィンドウを「固定（障害物）」と「移動対象」に分ける
    let mut obstacles = Vec::new();
    let mut targets = Vec::new();

    for win in all_windows {
        // ターゲットモニター上にあるか？
        let on_target = is_window_on_monitor(&win, target_monitor);
        if !on_target {
            info!("[arrange]   SKIP (別モニター) hwnd={} \"{}\" rect=({},{})–({},{})",
                win.hwnd, win.title,
                win.rect.left, win.rect.top, win.rect.right, win.rect.bottom);
            continue;
        }

        // --- リサイズ可否判定 ---
        // WS_THICKFRAME: サイズ変更枠
        const WS_THICKFRAME: u32 = 0x00040000;
        // WS_MAXIMIZEBOX / WS_MINIMIZEBOX の両方が欠落：ダイアログ等でリサイズ不可
        const WS_MAXIMIZEBOX: u32 = 0x00010000;
        const WS_MINIMIZEBOX: u32 = 0x00020000;

        let is_resizable = (win.style & WS_THICKFRAME) != 0
            || (win.style & WS_MAXIMIZEBOX) != 0
            || (win.style & WS_MINIMIZEBOX) != 0;

        let is_pinned   = crate::pin::is_pinned(win.hwnd);
        let is_excluded = crate::rule::is_excluded(&win, &parsed_rules);

        if is_pinned || is_excluded || !is_resizable {
            let reason = if is_pinned       { "ピン留め" }
                         else if is_excluded { "除外ルール" }
                         else               { "リサイズ不可" };
            info!("[arrange]   OBSTACLE ({}) hwnd={} \"{}\" rect=({},{})–({},{}) style=0x{:08X}",
                reason, win.hwnd, win.title,
                win.rect.left, win.rect.top, win.rect.right, win.rect.bottom,
                win.style);
            obstacles.push(win);
        } else {
            info!("[arrange]   TARGET hwnd={} \"{}\" rect=({},{})–({},{})",
                win.hwnd, win.title,
                win.rect.left, win.rect.top, win.rect.right, win.rect.bottom);
            targets.push(win);
        }
    }

    info!("[arrange] 分類結果 → obstacles={} 件 / targets={} 件",
        obstacles.len(), targets.len());

    if targets.is_empty() {
        info!("[arrange] 移動対象ウィンドウが0件のため整列を中断");
        return;
    }

    // Zオーダーを反転させる (EnumWindows は前面から列挙するため)
    // これにより、targets[0] が一番背面のウィンドウになり、左上奥に配置されるようになる
    targets.reverse();
    info!("[arrange] Zオーダーを反転しました (背面から前面の順)");

    // 3. 障害物を避けた有効な空き領域の計算
    let mut available_area = target_monitor.work_area.clone();
    
    // 単純化のため、障害物が存在する場合は、障害物の境界から
    // 最も面積が広くなるように available_area を削る簡易アルゴリズム
    for obs in &obstacles {
        // 障害物が現在の available_area と交差しているか
        if intersect(&available_area, &obs.rect) {
            // 4つの候補（上、下、左、右の空きスペース）を作成して最大のものを選ぶ
            let mut candidates = Vec::new();
            
            // 上側の空きスペース
            if obs.rect.top > available_area.top {
                candidates.push(MonitorRect {
                    left: available_area.left,
                    right: available_area.right,
                    top: available_area.top,
                    bottom: obs.rect.top,
                });
            }
            // 下側の空きスペース
            if obs.rect.bottom < available_area.bottom {
                candidates.push(MonitorRect {
                    left: available_area.left,
                    right: available_area.right,
                    top: obs.rect.bottom,
                    bottom: available_area.bottom,
                });
            }
            // 左側の空きスペース
            if obs.rect.left > available_area.left {
                candidates.push(MonitorRect {
                    left: available_area.left,
                    right: obs.rect.left,
                    top: available_area.top,
                    bottom: available_area.bottom,
                });
            }
            // 右側の空きスペース
            if obs.rect.right < available_area.right {
                candidates.push(MonitorRect {
                    left: obs.rect.right,
                    right: available_area.right,
                    top: available_area.top,
                    bottom: available_area.bottom,
                });
            }
            
            // 最大面積の候補を新しい available_area とする
            if let Some(best) = candidates.iter().max_by_key(|r| area(r)) {
                available_area = best.clone();
            }
        }
    }
    
    // 4. 配置計算
    let new_rects = calculate_layout(&targets, &available_area, arrange_type);

    // 5. 履歴保存と実行
    let history_state = app.state::<HistoryState>();
    let mut actions = Vec::new();

    for (i, win) in targets.iter().enumerate() {
        let new_rect = &new_rects[i];
        info!("[arrange]   MOVE hwnd={} \"{}\" ({},{})–({},{}) → ({},{})–({},{})",
            win.hwnd, win.title,
            win.rect.left, win.rect.top, win.rect.right, win.rect.bottom,
            new_rect.left, new_rect.top, new_rect.right, new_rect.bottom);
        actions.push(MoveAction {
            hwnd: win.hwnd,
            old_rect: win.rect.clone(),
            new_rect: new_rect.clone(),
        });

        unsafe {
            let result = SetWindowPos(
                windows::Win32::Foundation::HWND(win.hwnd as isize),
                HWND_TOP,
                new_rect.left,
                new_rect.top,
                new_rect.right - new_rect.left,
                new_rect.bottom - new_rect.top,
                SWP_NOACTIVATE,
            );
            if result.is_err() {
                info!("[arrange]   SetWindowPos 失敗: hwnd={}", win.hwnd);
            }
        }
    }
    history_state.push(actions);
    info!("[arrange] 完了: {} 件を整列 / モニター={} / 使用エリア=({},{})–({},{})",
        targets.len(), target_monitor.name,
        available_area.left, available_area.top,
        available_area.right, available_area.bottom);
}

fn intersect(r1: &MonitorRect, r2: &MonitorRect) -> bool {
    r1.left < r2.right && r1.right > r2.left && r1.top < r2.bottom && r1.bottom > r2.top
}

fn area(r: &MonitorRect) -> i32 {
    let w = (r.right - r.left).max(0);
    let h = (r.bottom - r.top).max(0);
    w * h
}

fn is_window_on_monitor(win: &WindowInfo, monitor: &MonitorInfo) -> bool {
    let intersect_left = win.rect.left.max(monitor.monitor_area.left);
    let intersect_right = win.rect.right.min(monitor.monitor_area.right);
    let intersect_top = win.rect.top.max(monitor.monitor_area.top);
    let intersect_bottom = win.rect.bottom.min(monitor.monitor_area.bottom);
    
    if intersect_right > intersect_left && intersect_bottom > intersect_top {
        let area = (intersect_right - intersect_left) * (intersect_bottom - intersect_top);
        let win_area = (win.rect.right - win.rect.left) * (win.rect.bottom - win.rect.top);
        if win_area <= 0 { return false; }
        // 半分以上重なっていればそのモニターにあるとみなす
        return area * 2 >= win_area;
    }
    false
}

fn get_active_monitor_index(monitors: &[MonitorInfo]) -> Option<usize> {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
        use windows::Win32::Graphics::Gdi::{MonitorFromWindow, MONITOR_DEFAULTTONEAREST};
        
        let hwnd = GetForegroundWindow();
        if hwnd.0 == 0 { return None; }
        
        let hmonitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        monitors.iter().position(|m| m.hmonitor == hmonitor.0 as isize)
    }
}

fn calculate_layout(targets: &[WindowInfo], area: &MonitorRect, arrange_type: ArrangeType) -> Vec<MonitorRect> {
    let n = targets.len();
    let mut rects = Vec::with_capacity(n);
    let width = area.right - area.left;
    let height = area.bottom - area.top;

    match arrange_type {
        ArrangeType::Grid => {
            let cols = (n as f64).sqrt().ceil() as i32;
            let rows = (n as f64 / cols as f64).ceil() as i32;
            let cell_w = width / cols;
            let cell_h = height / rows;

            for i in 0..n {
                let r = i as i32 / cols;
                let c = i as i32 % cols;
                rects.push(MonitorRect {
                    left: area.left + c * cell_w,
                    top: area.top + r * cell_h,
                    right: area.left + (c + 1) * cell_w,
                    bottom: area.top + (r + 1) * cell_h,
                });
            }
        }
        ArrangeType::Vertical => {
            let cell_h = height / n as i32;
            for i in 0..n {
                rects.push(MonitorRect {
                    left: area.left,
                    top: area.top + i as i32 * cell_h,
                    right: area.right,
                    bottom: area.top + (i as i32 + 1) * cell_h,
                });
            }
        }
        ArrangeType::Horizontal => {
            let cell_w = width / n as i32;
            for i in 0..n {
                rects.push(MonitorRect {
                    left: area.left + i as i32 * cell_w,
                    top: area.top,
                    right: area.left + (i as i32 + 1) * cell_w,
                    bottom: area.bottom,
                });
            }
        }
        ArrangeType::Cascade => {
            let offset = 30; // 階段状のずらし幅
            // 元のサイズの70%程度にする
            let win_w = (width as f64 * 0.7) as i32;
            let win_h = (height as f64 * 0.7) as i32;

            for i in 0..n {
                let x = area.left + (i as i32 * offset);
                let y = area.top + (i as i32 * offset);
                rects.push(MonitorRect {
                    left: x,
                    top: y,
                    right: x + win_w,
                    bottom: y + win_h,
                });
            }
        }
    }
    rects
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::MonitorRect;
    use crate::window::WindowInfo;

    // ---------------------------------------------------------
    // テスト用ヘルパー
    // ---------------------------------------------------------
    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> MonitorRect {
        MonitorRect { left, top, right, bottom }
    }

    fn mock_monitor(left: i32, top: i32, right: i32, bottom: i32) -> crate::monitor::MonitorInfo {
        crate::monitor::MonitorInfo {
            hmonitor: 1,
            name: "DISPLAY1".to_string(),
            display_number: 1,
            manufacturer: "Test".to_string(),
            serial_number: "000".to_string(),
            work_area: rect(left, top, right, bottom),
            monitor_area: rect(left, top, right, bottom),
            is_primary: true,
        }
    }

    fn mock_window(l: i32, t: i32, r: i32, b: i32) -> WindowInfo {
        WindowInfo {
            hwnd: 0,
            title: "Test Window".to_string(),
            rect: rect(l, t, r, b),
            dpi: 96,
            is_maximized: false,
            is_minimized: false,
            is_fullscreen: false,
            process_name: "test.exe".to_string(),
            class_name: "TestClass".to_string(),
            style: 0x00CF0000, // WS_OVERLAPPEDWINDOW
            ex_style: 0,
        }
    }

    // ---------------------------------------------------------
    // area() のテスト
    // ---------------------------------------------------------
    #[test]
    fn test_area_normal() {
        let r = rect(0, 0, 100, 200);
        assert_eq!(area(&r), 20000);
    }

    #[test]
    fn test_area_zero_width() {
        let r = rect(50, 0, 50, 100);
        assert_eq!(area(&r), 0);
    }

    #[test]
    fn test_area_negative_size() {
        // right < left の場合は 0
        let r = rect(100, 0, 50, 100);
        assert_eq!(area(&r), 0);
    }

    // ---------------------------------------------------------
    // intersect() のテスト
    // ---------------------------------------------------------
    #[test]
    fn test_intersect_overlap() {
        let r1 = rect(0, 0, 100, 100);
        let r2 = rect(50, 50, 150, 150);
        assert!(intersect(&r1, &r2));
    }

    #[test]
    fn test_intersect_no_overlap_horizontal() {
        // 右端と左端がぴったり接するだけ → 交差なし
        let r1 = rect(0, 0, 100, 100);
        let r2 = rect(100, 0, 200, 100);
        assert!(!intersect(&r1, &r2));
    }

    #[test]
    fn test_intersect_no_overlap_vertical() {
        // 下端と上端がぴったり接するだけ → 交差なし
        let r1 = rect(0, 0, 100, 100);
        let r2 = rect(0, 100, 100, 200);
        assert!(!intersect(&r1, &r2));
    }

    #[test]
    fn test_intersect_contained() {
        // r2 が r1 に完全に含まれる場合
        let r1 = rect(0, 0, 200, 200);
        let r2 = rect(50, 50, 150, 150);
        assert!(intersect(&r1, &r2));
    }

    #[test]
    fn test_intersect_far_apart() {
        let r1 = rect(0, 0, 100, 100);
        let r2 = rect(500, 500, 600, 600);
        assert!(!intersect(&r1, &r2));
    }

    // ---------------------------------------------------------
    // is_window_on_monitor() のテスト
    // ---------------------------------------------------------
    #[test]
    fn test_window_on_monitor_fully_inside() {
        let monitor = mock_monitor(0, 0, 1920, 1080);
        let win = mock_window(100, 100, 800, 600);
        assert!(is_window_on_monitor(&win, &monitor));
    }

    #[test]
    fn test_window_completely_outside_monitor() {
        // ウィンドウが完全にモニターの右外にある
        let monitor = mock_monitor(0, 0, 1920, 1080);
        let win = mock_window(2000, 0, 2500, 1080);
        assert!(!is_window_on_monitor(&win, &monitor));
    }

    #[test]
    fn test_window_mostly_outside_monitor() {
        // ウィンドウの大部分がモニター外（左端 200 ピクセルだけ内側）
        // 幅: 1200, 内側: 200 → 200/1200 < 50% → false
        let monitor = mock_monitor(0, 0, 1920, 1080);
        let win = mock_window(-1000, 0, 200, 1080);
        assert!(!is_window_on_monitor(&win, &monitor));
    }

    // ---------------------------------------------------------
    // calculate_layout() のテスト
    // ---------------------------------------------------------
    fn make_windows(n: usize) -> Vec<WindowInfo> {
        (0..n).map(|_| mock_window(0, 0, 400, 300)).collect()
    }

    #[test]
    fn test_layout_vertical_2() {
        let area = rect(0, 0, 1920, 1080);
        let wins = make_windows(2);
        let rects = calculate_layout(&wins, &area, ArrangeType::Vertical);
        assert_eq!(rects.len(), 2);
        // 上半分
        assert_eq!(rects[0].top, 0);
        assert_eq!(rects[0].bottom, 540);
        // 下半分
        assert_eq!(rects[1].top, 540);
        assert_eq!(rects[1].bottom, 1080);
        // 左右はフル幅
        assert_eq!(rects[0].left, 0);
        assert_eq!(rects[0].right, 1920);
    }

    #[test]
    fn test_layout_horizontal_3() {
        let area = rect(0, 0, 1920, 1080);
        let wins = make_windows(3);
        let rects = calculate_layout(&wins, &area, ArrangeType::Horizontal);
        assert_eq!(rects.len(), 3);
        let cell_w = 1920 / 3;
        assert_eq!(rects[0].left, 0);
        assert_eq!(rects[0].right, cell_w);
        assert_eq!(rects[1].left, cell_w);
        assert_eq!(rects[2].right, 1920);
        // 上下はフル高さ
        for r in &rects {
            assert_eq!(r.top, 0);
            assert_eq!(r.bottom, 1080);
        }
    }

    #[test]
    fn test_layout_grid_4() {
        let area = rect(0, 0, 1920, 1080);
        let wins = make_windows(4);
        let rects = calculate_layout(&wins, &area, ArrangeType::Grid);
        assert_eq!(rects.len(), 4);
        // 4 枚 → 2×2 グリッド
        let cols = 2_i32;
        let cell_w = 1920 / cols;
        let cell_h = 1080 / cols;
        // 左上
        assert_eq!(rects[0], rect(0, 0, cell_w, cell_h));
        // 右上
        assert_eq!(rects[1].left, cell_w);
        assert_eq!(rects[1].top, 0);
        // 左下
        assert_eq!(rects[2].left, 0);
        assert_eq!(rects[2].top, cell_h);
    }

    #[test]
    fn test_layout_cascade_offsets() {
        let area = rect(0, 0, 1920, 1080);
        let wins = make_windows(3);
        let rects = calculate_layout(&wins, &area, ArrangeType::Cascade);
        assert_eq!(rects.len(), 3);
        let offset = 30;
        // 各ウィンドウは前より offset ずつ右下にずれる
        assert_eq!(rects[0].left, 0);
        assert_eq!(rects[0].top, 0);
        assert_eq!(rects[1].left, offset);
        assert_eq!(rects[1].top, offset);
        assert_eq!(rects[2].left, offset * 2);
        assert_eq!(rects[2].top, offset * 2);
    }

    #[test]
    fn test_layout_single_window_grid_fills_area() {
        let area = rect(100, 200, 1820, 880);
        let wins = make_windows(1);
        let rects = calculate_layout(&wins, &area, ArrangeType::Grid);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0], rect(100, 200, 1820, 880));
    }
}
