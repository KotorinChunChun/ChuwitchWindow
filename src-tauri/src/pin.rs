//! ウィンドウの一時的なピン留め管理 (SPEC 2-1)
//!
//! ピン留めされたウィンドウは、モニター間移動（シフト・入れ替え）の
//! 対象から除外される。ピン留め状態はメモリ上のみで管理し、
//! アプリ終了時に自動的にクリアされる（永続化しない）。

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

/// ピン留め中の HWND を格納するグローバルセット
static PINNED_WINDOWS: OnceLock<Mutex<HashSet<isize>>> = OnceLock::new();

/// 内部ヘルパー: PINNED_WINDOWS を確実に初期化して返す
fn pinned_set() -> &'static Mutex<HashSet<isize>> {
    PINNED_WINDOWS.get_or_init(|| Mutex::new(HashSet::new()))
}

/// 現在フォアグラウンドにあるウィンドウのピン留め状態をトグルする
///
/// - ピン留めされていなければピン留めする
/// - すでにピン留め済みならピン留めを解除する
pub fn toggle_pin() {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0 == 0 {
        return; // フォアグラウンドウィンドウが取得できない場合は何もしない
    }
    let hwnd_val = hwnd.0 as isize;
    let mut set = pinned_set().lock().unwrap();
    if set.contains(&hwnd_val) {
        set.remove(&hwnd_val);
        tracing::info!("ピン留め解除: HWND={}", hwnd_val);
    } else {
        set.insert(hwnd_val);
        tracing::info!("ピン留め設定: HWND={}", hwnd_val);
    }
}

/// 指定 HWND がピン留めされているか判定する
pub fn is_pinned(hwnd: isize) -> bool {
    let set = pinned_set().lock().unwrap();
    set.contains(&hwnd)
}

/// 現在ピン留めされている全 HWND のリストを返す
/// （除外ルール UI でのピン留め提案機能用）
pub fn get_pinned_list() -> Vec<isize> {
    let set = pinned_set().lock().unwrap();
    set.iter().cloned().collect()
}

/// 指定 HWND のピン留めを解除する（プロセス終了時のクリーンアップ等）
pub fn remove_pin(hwnd: isize) {
    let mut set = pinned_set().lock().unwrap();
    set.remove(&hwnd);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pin_toggle_and_check() {
        // 直接 HWND を指定してテスト（toggle_pin は GetForegroundWindow 依存のため直接テストしない）
        let test_hwnd: isize = 12345;

        // 初期状態: ピン留めされていない
        assert!(!is_pinned(test_hwnd));

        // ピン留め追加
        {
            let mut set = pinned_set().lock().unwrap();
            set.insert(test_hwnd);
        }
        assert!(is_pinned(test_hwnd));

        // ピン留め解除
        remove_pin(test_hwnd);
        assert!(!is_pinned(test_hwnd));
    }

    #[test]
    fn test_get_pinned_list() {
        let hwnd_a: isize = 99999;
        let hwnd_b: isize = 88888;

        {
            let mut set = pinned_set().lock().unwrap();
            set.insert(hwnd_a);
            set.insert(hwnd_b);
        }

        let list = get_pinned_list();
        assert!(list.contains(&hwnd_a));
        assert!(list.contains(&hwnd_b));

        // クリーンアップ
        remove_pin(hwnd_a);
        remove_pin(hwnd_b);
    }
}
