use crate::layout::{compute_layout, Size};
use crate::monitor::{MonitorInfo, MonitorRect};
use crate::window::{get_window_frame, WindowFrame, WindowInfo};
use crate::history::HistoryState;
use tauri::Manager;
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

/// ポップアップで記憶した対象モニターを取り出す（取り出したら消す）
pub fn take_target_hmonitor() -> Option<isize> {
    last_target_hmonitor_lock().lock().unwrap().take()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ArrangeType {
    Grid,
    Vertical,
    Horizontal,
    Cascade,
}

impl ArrangeType {
    /// 整列方法の名前（"Grid" / "grid" など、大文字小文字は問わない）を解釈する
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "grid" => Some(Self::Grid),
            "vertical" => Some(Self::Vertical),
            "horizontal" => Some(Self::Horizontal),
            "cascade" => Some(Self::Cascade),
            _ => None,
        }
    }
}

/// 実測した最小サイズで配置をやり直す最大回数
const MAX_RELAYOUT: usize = 2;
/// 割り当てたサイズより実際のサイズが大きいとみなす誤差（ピクセル）
const SIZE_TOLERANCE: i32 = 2;

/// 自動整列を実行する。
/// `target_hmonitor` が `None` なら、フォアグラウンドウィンドウのあるモニターを対象にする。
pub fn handle_arrange(app: &tauri::AppHandle, arrange_type: ArrangeType, target_hmonitor: Option<isize>) {
    info!("Exec [handle_arrange]: type={:?}", arrange_type);

    let monitors = crate::monitor::get_all_monitors();
    if monitors.is_empty() {
        info!("[arrange] モニターが0件のため終了");
        return;
    }

    // ポップアップから実行された場合は、表示時に記憶した hmonitor を使う。
    // 表示後は GetForegroundWindow() が arrange ウィンドウ自身を返してしまうため、
    // その段階で呼ぶと対象モニターの判定が狂う。
    let stored_idx = target_hmonitor.and_then(|hmon| monitors.iter().position(|m| m.hmonitor == hmon));
    let active_idx = match stored_idx {
        Some(idx) => {
            info!("[arrange] 記憶済みhmonitor → monitors[{}]={} を対象に決定", idx, monitors[idx].name);
            idx
        }
        None => {
            let idx = crate::monitor::get_active_monitor_index(&monitors).unwrap_or(0);
            info!("[arrange] GetForegroundWindow() → monitors[{}]={} を対象に決定", idx, monitors[idx].name);
            idx
        }
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

    // 3. 履歴保存（最大化の解除より前に、操作前の状態を記録する）
    app.state::<HistoryState>().push_windows(&targets);

    // 最大化したままでは並べた位置に収まらないため、通常表示に戻してから枠の大きさを測る
    for win in targets.iter().filter(|w| w.is_maximized) {
        crate::window::restore_window(win.hwnd);
    }

    // 4. 配置計算は「見た目の枠」で行う（Windows 10/11 の不可視のリサイズ枠のぶん隙間が空かないように）
    let frames: Vec<WindowFrame> = targets.iter().map(|w| get_window_frame(w.hwnd)).collect();
    let obstacle_rects: Vec<MonitorRect> = obstacles.iter().map(|w| get_window_frame(w.hwnd).visible).collect();
    let dpi = targets[0].dpi.max(96);
    let mut mins: Vec<Size> = frames.iter().map(|f| system_min_size(f, dpi)).collect();
    let cascade_offset = cascade_offset(dpi);

    // 5. 配置。ウィンドウ側の最小サイズに阻まれて割り当てより大きくなったものがあれば、
    //    実際の大きさを最小サイズとして配置し直す。
    let mut layout = Vec::new();
    for attempt in 0..=MAX_RELAYOUT {
        layout = compute_layout(&target_monitor.work_area, &obstacle_rects, &mins, arrange_type, cascade_offset);

        // targets は前面から順。背面から順に HWND_TOP で置くことで、元の Z オーダーを保つ
        for (i, win) in targets.iter().enumerate().rev() {
            let visible = &layout[i];
            info!("[arrange]   MOVE hwnd={} \"{}\" → ({},{})–({},{})",
                win.hwnd, win.title, visible.left, visible.top, visible.right, visible.bottom);
            crate::window::set_window_rect(win.hwnd, &frames[i].to_window_rect(visible));
        }

        let mut changed = false;
        for (i, win) in targets.iter().enumerate() {
            let actual = get_window_frame(win.hwnd).visible;
            let (actual_w, actual_h) = (actual.right - actual.left, actual.bottom - actual.top);
            let (planned_w, planned_h) = (layout[i].right - layout[i].left, layout[i].bottom - layout[i].top);
            if actual_w > planned_w + SIZE_TOLERANCE && actual_w > mins[i].w {
                mins[i].w = actual_w;
                changed = true;
            }
            if actual_h > planned_h + SIZE_TOLERANCE && actual_h > mins[i].h {
                mins[i].h = actual_h;
                changed = true;
            }
        }
        if !changed {
            break;
        }
        info!("[arrange] 最小サイズに阻まれたウィンドウがあるため配置し直す (attempt={})", attempt + 1);
    }

    info!("[arrange] 完了: {} 件を整列 / モニター={} / 固定ウィンドウ={} 件 / 配置={:?}",
        targets.len(), target_monitor.name, obstacle_rects.len(), layout);
}

/// システムが許すウィンドウの最小サイズ（見た目の枠の大きさに換算）。
/// アプリ独自の最小サイズは、実際に配置してから測って反映する。
fn system_min_size(frame: &WindowFrame, dpi: u32) -> Size {
    use windows::Win32::UI::HiDpi::GetSystemMetricsForDpi;
    use windows::Win32::UI::WindowsAndMessaging::{SM_CXMINTRACK, SM_CYMINTRACK};
    let (min_w, min_h) = unsafe {
        (GetSystemMetricsForDpi(SM_CXMINTRACK, dpi), GetSystemMetricsForDpi(SM_CYMINTRACK, dpi))
    };
    let (margin_w, margin_h) = frame.margin_size();
    Size { w: (min_w - margin_w).max(0), h: (min_h - margin_h).max(0) }
}

/// 重ねて表示でずらす量。タイトルバーがちょうど見える高さにする。
fn cascade_offset(dpi: u32) -> i32 {
    use windows::Win32::UI::HiDpi::GetSystemMetricsForDpi;
    use windows::Win32::UI::WindowsAndMessaging::{SM_CXPADDEDBORDER, SM_CYCAPTION, SM_CYSIZEFRAME};
    let offset = unsafe {
        GetSystemMetricsForDpi(SM_CYCAPTION, dpi)
            + GetSystemMetricsForDpi(SM_CYSIZEFRAME, dpi)
            + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi)
    };
    if offset > 0 { offset } else { 30 }
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
    // ArrangeType::parse() のテスト
    // ---------------------------------------------------------
    #[test]
    fn test_parse_arrange_type() {
        assert_eq!(ArrangeType::parse("Grid"), Some(ArrangeType::Grid));
        assert_eq!(ArrangeType::parse("vertical"), Some(ArrangeType::Vertical));
        assert_eq!(ArrangeType::parse("HORIZONTAL"), Some(ArrangeType::Horizontal));
        assert_eq!(ArrangeType::parse("cascade"), Some(ArrangeType::Cascade));
        assert_eq!(ArrangeType::parse("tile"), None);
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
}
