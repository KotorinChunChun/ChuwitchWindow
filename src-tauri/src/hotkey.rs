use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager,
};
use std::str::FromStr;
use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::AppHandle;

use crate::action::Action;
use crate::config::AppConfig;

static IS_RECORDING: AtomicBool = AtomicBool::new(false);

struct ManagerWrapper(GlobalHotKeyManager);
unsafe impl Send for ManagerWrapper {}
unsafe impl Sync for ManagerWrapper {}

static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();
static MANAGER: OnceLock<Mutex<ManagerWrapper>> = OnceLock::new();
/// 登録に成功したホットキーと、押されたときに実行する操作
static BINDINGS: OnceLock<Mutex<Vec<(HotKey, Action)>>> = OnceLock::new();

pub fn init(app: AppHandle) {
    if APP_HANDLE.set(app).is_err() {
        return;
    }

    let manager = match GlobalHotKeyManager::new() {
        Ok(manager) => manager,
        Err(e) => {
            tracing::error!("GlobalHotKeyManager の初期化に失敗しました: {}", e);
            return;
        }
    };
    let _ = MANAGER.set(Mutex::new(ManagerWrapper(manager)));
    let _ = BINDINGS.set(Mutex::new(Vec::new()));

    reload_hotkeys();

    let receiver = GlobalHotKeyEvent::receiver();

    std::thread::spawn(move || {
        while let Ok(event) = receiver.recv() {
            if event.state != global_hotkey::HotKeyState::Pressed {
                continue;
            }
            if IS_RECORDING.load(Ordering::SeqCst) {
                continue; // 録画中（設定中）はアプリのショートカットを無視
            }
            // ロックを保持したまま操作を実行しないよう、先に操作だけ取り出す
            let action = BINDINGS.get().and_then(|bindings| {
                bindings.lock().unwrap()
                    .iter()
                    .find(|(hotkey, _)| hotkey.id() == event.id)
                    .map(|(_, action)| *action)
            });
            if let (Some(action), Some(app_handle)) = (action, APP_HANDLE.get()) {
                crate::action::execute(app_handle, action);
            }
        }
    });
}

/// 設定からホットキーと操作の対応表を作る。
/// 同じキーが複数の操作に割り当てられている場合は、先に並んでいる操作が優先される。
fn build_bindings(config: &AppConfig) -> Vec<(HotKey, Action)> {
    let mut entries = vec![
        (config.rotate_cw_hotkey.clone(), Action::RotateCw),
        (config.rotate_ccw_hotkey.clone(), Action::RotateCcw),
        (config.undo_hotkey.clone(), Action::Undo),
        // v0.2
        (config.pin_hotkey.clone(), Action::PinToggle),
        (config.escape_hotkey.clone(), Action::Escape),
        (config.gather_hotkey.clone(), Action::Gather),
        (config.arrange_hotkey.clone(), Action::Arrange),
    ];
    // 修飾キー + 2~9 で指定モニターと入れ替え
    if !config.swap_target_modifiers.is_empty() {
        for target_num in 2..=9 {
            entries.push((
                format!("{}+{}", config.swap_target_modifiers, target_num),
                Action::SwapTarget(target_num),
            ));
        }
    }

    let mut bindings: Vec<(HotKey, Action)> = Vec::new();
    for (hotkey_str, action) in entries {
        if let Some(hotkey) = parse_hotkey(&hotkey_str) {
            if !bindings.iter().any(|(k, _)| k.id() == hotkey.id()) {
                bindings.push((hotkey, action));
            }
        }
    }
    bindings
}

pub fn reload_hotkeys() {
    let config = crate::config::load_config();
    let new_bindings = build_bindings(&config);

    if let (Some(manager_lock), Some(bindings_lock)) = (MANAGER.get(), BINDINGS.get()) {
        let manager_wrapper = manager_lock.lock().unwrap();
        let manager = &manager_wrapper.0;
        let mut current = bindings_lock.lock().unwrap();

        for (hotkey, _) in current.iter() {
            let _ = manager.unregister(*hotkey);
        }
        current.clear();

        for (hotkey, action) in new_bindings {
            match manager.register(hotkey) {
                Ok(()) => current.push((hotkey, action)),
                Err(e) => tracing::warn!("ホットキーの登録に失敗しました ({:?}): {}", action, e),
            }
        }
    }
}

fn parse_hotkey(hotkey_str: &str) -> Option<HotKey> {
    if hotkey_str.is_empty() {
        return None;
    }

    let mut modifiers = Modifiers::empty();
    let mut key: Option<&str> = None;

    for part in hotkey_str.split('+') {
        let part = part.trim();
        match part.to_uppercase().as_str() {
            "SUPER" | "WIN" | "CMD" | "META" => modifiers.insert(Modifiers::SUPER),
            "CONTROL" | "CTRL" => modifiers.insert(Modifiers::CONTROL),
            "ALT" | "OPTION" => modifiers.insert(Modifiers::ALT),
            "SHIFT" => modifiers.insert(Modifiers::SHIFT),
            _ => key = Some(part),
        }
    }

    let code = parse_key_code(key?)?;
    Some(HotKey::new(if modifiers.is_empty() { None } else { Some(modifiers) }, code))
}

/// キー名をキーコードに変換する。
/// 設定画面が記録する短縮名（"A", "1", "Right" 等）は大文字小文字を区別せずに受け付け、
/// それ以外は KeyboardEvent.code と同じ名前（"F1", "Tab", "KeyA" 等）として解釈する。
fn parse_key_code(key: &str) -> Option<Code> {
    let upper = key.to_uppercase();
    // 誤って記録された "DIGIT1" / "KEYA" 形式の接頭辞を取り除く
    let base = upper
        .strip_prefix("DIGIT")
        .or_else(|| upper.strip_prefix("KEY"))
        .filter(|rest| rest.len() == 1)
        .unwrap_or(&upper);

    let code = match base {
        "ARROWRIGHT" | "RIGHT" => Code::ArrowRight,
        "ARROWLEFT" | "LEFT" => Code::ArrowLeft,
        "ARROWUP" | "UP" => Code::ArrowUp,
        "ARROWDOWN" | "DOWN" => Code::ArrowDown,
        "SPACE" => Code::Space,
        "ENTER" => Code::Enter,
        "ESCAPE" | "ESC" => Code::Escape,
        s if s.len() == 1 => {
            let c = s.chars().next()?;
            let name = if c.is_ascii_digit() {
                format!("Digit{}", c)
            } else if c.is_ascii_uppercase() {
                format!("Key{}", c)
            } else {
                return None;
            };
            return Code::from_str(&name).ok();
        }
        _ => return Code::from_str(key).ok().filter(|c| *c != Code::Unidentified),
    };
    Some(code)
}

pub fn check_hotkey_conflict(hotkey_str: String) -> bool {
    // Attempt to register it temporarily via global_hotkey
    if let Some(hk) = parse_hotkey(&hotkey_str) {
        // もしアプリ自身がすでに登録しているホットキーの場合は、競合とみなさない
        if let Some(bindings_lock) = BINDINGS.get() {
            if let Ok(current) = bindings_lock.lock() {
                if current.iter().any(|(k, _)| k.id() == hk.id()) {
                    return false;
                }
            }
        }

        if let Ok(manager) = GlobalHotKeyManager::new() {
            if manager.register(hk).is_ok() {
                let _ = manager.unregister(hk);
                return false; // No conflict
            } else {
                return true; // Conflict
            }
        }
    }
    true // If we can't parse or instantiate manager, consider it conflicted or invalid
}

pub fn set_recording_state(is_recording: bool) {
    IS_RECORDING.store(is_recording, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hotkey(mods: Modifiers, code: Code) -> HotKey {
        HotKey::new(Some(mods), code)
    }

    const WIN_CTRL_ALT: Modifiers = Modifiers::SUPER.union(Modifiers::CONTROL).union(Modifiers::ALT);

    #[test]
    fn test_parse_default_hotkeys() {
        assert_eq!(parse_hotkey("Win+Ctrl+Alt+Right"), Some(hotkey(WIN_CTRL_ALT, Code::ArrowRight)));
        assert_eq!(parse_hotkey("Win+Ctrl+Alt+Left"), Some(hotkey(WIN_CTRL_ALT, Code::ArrowLeft)));
        assert_eq!(parse_hotkey("Win+Ctrl+Alt+Z"), Some(hotkey(WIN_CTRL_ALT, Code::KeyZ)));
        assert_eq!(parse_hotkey("Win+Ctrl+Alt+2"), Some(hotkey(WIN_CTRL_ALT, Code::Digit2)));
    }

    #[test]
    fn test_parse_is_case_insensitive_and_accepts_legacy_prefix() {
        let expected = Some(hotkey(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyA));
        assert_eq!(parse_hotkey("ctrl+shift+a"), expected);
        assert_eq!(parse_hotkey("Ctrl+Shift+KeyA"), expected);
        assert_eq!(parse_hotkey("Ctrl+Digit5"), Some(hotkey(Modifiers::CONTROL, Code::Digit5)));
        assert_eq!(parse_hotkey("Alt+arrowup"), Some(hotkey(Modifiers::ALT, Code::ArrowUp)));
    }

    #[test]
    fn test_parse_keyboard_event_code_names() {
        // 設定画面が e.code をそのまま記録するキー
        assert_eq!(parse_hotkey("Ctrl+Alt+F1"), Some(hotkey(Modifiers::CONTROL | Modifiers::ALT, Code::F1)));
        assert_eq!(parse_hotkey("Win+Tab"), Some(hotkey(Modifiers::SUPER, Code::Tab)));
        assert_eq!(parse_hotkey("Ctrl+Escape"), Some(hotkey(Modifiers::CONTROL, Code::Escape)));
    }

    #[test]
    fn test_parse_invalid() {
        assert_eq!(parse_hotkey(""), None);
        assert_eq!(parse_hotkey("Win+Ctrl+Alt"), None); // キー本体なし
        assert_eq!(parse_hotkey("Ctrl+NoSuchKey"), None);
    }

    #[test]
    fn test_build_bindings_defaults() {
        let bindings = build_bindings(&AppConfig::default());
        let find = |action: Action| bindings.iter().find(|(_, a)| *a == action).map(|(k, _)| *k);

        assert_eq!(find(Action::RotateCw), Some(hotkey(WIN_CTRL_ALT, Code::ArrowRight)));
        assert_eq!(find(Action::Arrange), Some(hotkey(WIN_CTRL_ALT, Code::KeyA)));
        assert_eq!(find(Action::SwapTarget(9)), Some(hotkey(WIN_CTRL_ALT, Code::Digit9)));
        // 7 操作 + 入れ替え 2~9 の 8 個
        assert_eq!(bindings.len(), 15);
    }

    #[test]
    fn test_build_bindings_duplicate_key_keeps_first() {
        let config = AppConfig {
            gather_hotkey: "Win+Ctrl+Alt+E".to_string(), // escape と同じキー
            swap_target_modifiers: String::new(),
            ..AppConfig::default()
        };
        let bindings = build_bindings(&config);
        let escape_key = parse_hotkey("Win+Ctrl+Alt+E").unwrap();
        let matched: Vec<Action> = bindings.iter()
            .filter(|(k, _)| k.id() == escape_key.id())
            .map(|(_, a)| *a)
            .collect();
        assert_eq!(matched, [Action::Escape]);
        // 修飾キー未設定なら入れ替えキーは登録しない
        assert!(!bindings.iter().any(|(_, a)| matches!(a, Action::SwapTarget(_))));
    }
}
